use crate::{message, AnnouncedConsumer, Error, Path, RouterConsumer, Track, TrackConsumer};

use moq_async::{spawn, Close, OrClose};

mod publisher;
mod reader;
mod stream;
mod subscriber;
mod writer;

use publisher::*;
use reader::*;
use stream::*;
use subscriber::*;
use writer::*;

/// A MoqTransfork session, used to publish and/or subscribe to broadcasts.
///
/// A publisher will [Self::publish] tracks, or alternatively [Self::announce] and [Self::route] arbitrary paths.
/// A subscriber will [Self::subscribe] to tracks, or alternatively use [Self::announced] to discover arbitrary paths.
#[derive(Clone)]
pub struct Session {
	webtransport: web_transport::Session,
	publisher: Publisher,
	subscriber: Subscriber,
}

impl Session {
	fn new(mut session: web_transport::Session, stream: Stream) -> Self {
		let publisher = Publisher::new(session.clone());
		let subscriber = Subscriber::new(session.clone());

		let this = Self {
			webtransport: session.clone(),
			publisher: publisher.clone(),
			subscriber: subscriber.clone(),
		};

		spawn(async move {
			let res = tokio::select! {
				res = Self::run_session(stream) => res,
				res = Self::run_bi(session.clone(), publisher) => res,
				res = Self::run_uni(session.clone(), subscriber) => res,
			};

			if let Err(err) = res {
				tracing::warn!(?err, "terminated");
				session.close(err.to_code(), &err.to_string());
			}
		});

		this
	}

	/// Perform the MoQ handshake as a client.
	pub async fn connect<T: Into<web_transport::Session>>(session: T) -> Result<Self, Error> {
		let mut session = session.into();
		let mut stream = Stream::open(&mut session, message::ControlType::Session).await?;
		Self::connect_setup(&mut stream).await.or_close(&mut stream)?;
		Ok(Self::new(session, stream))
	}

	async fn connect_setup(setup: &mut Stream) -> Result<(), Error> {
		let client = message::ClientSetup {
			versions: [message::Version::CURRENT].into(),
			extensions: Default::default(),
		};

		setup.writer.encode(&client).await?;
		let server: message::ServerSetup = setup.reader.decode().await?;

		tracing::info!(version = ?server.version, "connected");

		Ok(())
	}

	/// Perform the MoQ handshake as a server
	pub async fn accept<T: Into<web_transport::Session>>(session: T) -> Result<Self, Error> {
		let mut session = session.into();
		let mut stream = Stream::accept(&mut session).await?;
		let kind = stream.reader.decode().await?;

		if kind != message::ControlType::Session {
			return Err(Error::UnexpectedStream(kind));
		}

		Self::accept_setup(&mut stream).await.or_close(&mut stream)?;
		Ok(Self::new(session, stream))
	}

	async fn accept_setup(control: &mut Stream) -> Result<(), Error> {
		let client: message::ClientSetup = control.reader.decode().await?;

		if !client.versions.contains(&message::Version::CURRENT) {
			return Err(Error::Version(client.versions, [message::Version::CURRENT].into()));
		}

		let server = message::ServerSetup {
			version: message::Version::CURRENT,
			extensions: Default::default(),
		};

		control.writer.encode(&server).await?;

		tracing::info!(version = ?server.version, "connected");

		Ok(())
	}

	async fn run_session(mut stream: Stream) -> Result<(), Error> {
		while let Some(_info) = stream.reader.decode_maybe::<message::Info>().await? {}
		Err(Error::Cancel)
	}

	async fn run_uni(mut session: web_transport::Session, subscriber: Subscriber) -> Result<(), Error> {
		loop {
			let mut stream = Reader::accept(&mut session).await?;
			let subscriber = subscriber.clone();

			spawn(async move {
				Self::run_data(&mut stream, subscriber).await.or_close(&mut stream).ok();
			});
		}
	}

	async fn run_data(stream: &mut Reader, mut subscriber: Subscriber) -> Result<(), Error> {
		let kind = stream.decode().await?;
		match kind {
			message::DataType::Group => subscriber.recv_group(stream).await,
		}
	}

	async fn run_bi(mut session: web_transport::Session, publisher: Publisher) -> Result<(), Error> {
		loop {
			let mut stream = Stream::accept(&mut session).await?;
			let publisher = publisher.clone();

			spawn(async move {
				Self::run_control(&mut stream, publisher)
					.await
					.or_close(&mut stream)
					.ok();
			});
		}
	}

	async fn run_control(stream: &mut Stream, mut publisher: Publisher) -> Result<(), Error> {
		let kind = stream.reader.decode().await?;
		match kind {
			message::ControlType::Session => Err(Error::UnexpectedStream(kind)),
			message::ControlType::Announce => publisher.recv_announce(stream).await,
			message::ControlType::Subscribe => publisher.recv_subscribe(stream).await,
			message::ControlType::Fetch => publisher.recv_fetch(stream).await,
			message::ControlType::Info => publisher.recv_info(stream).await,
		}
	}

	/// Publish a track, automatically announcing and serving it.
	pub fn publish(&mut self, track: TrackConsumer) -> Result<(), Error> {
		self.publisher.publish(track)
	}

	/// Optionally announce the provided tracks.
	/// This is advanced functionality if you wish to perform dynamic track generation in conjunction with [Self::route].
	pub fn announce(&mut self, announced: AnnouncedConsumer) {
		self.publisher.announce(announced);
	}

	/// Optionally route unknown paths.
	/// This is advanced functionality if you wish to perform dynamic track generation in conjunction with [Self::announce].
	pub fn route(&mut self, router: RouterConsumer) {
		self.publisher.route(router);
	}

	/// Subscribe to a track and start receiving data over the network.
	pub fn subscribe(&self, track: Track) -> TrackConsumer {
		self.subscriber.subscribe(track)
	}

	/// Discover any tracks published by the remote matching a prefix.
	pub fn announced(&self, prefix: Path) -> AnnouncedConsumer {
		self.subscriber.announced(prefix)
	}

	/// Close the underlying WebTransport session.
	pub fn close(mut self, err: Error) {
		self.webtransport.close(err.to_code(), &err.to_string());
	}

	/// Block until the WebTransport session is closed.
	pub async fn closed(&self) -> Error {
		self.webtransport.closed().await.into()
	}
}

impl PartialEq for Session {
	fn eq(&self, other: &Self) -> bool {
		self.webtransport == other.webtransport
	}
}

impl Eq for Session {}
