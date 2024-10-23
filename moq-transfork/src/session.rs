use crate::{
	message,
	util::{spawn, Close, OrClose},
	AnnouncedConsumer, Broadcast, BroadcastConsumer, Error, Path,
};

mod publisher;
mod reader;
mod stream;
mod subscribe;
mod subscriber;
mod writer;

use publisher::*;
use reader::*;
use stream::*;
use subscribe::*;
use subscriber::*;
use writer::*;

/// A MoqTransfork session, used to publish and/or subscribe to broadcasts.
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
				tracing::warn!(?err, "closing session");
				session.close(err.to_code(), &err.to_string());
			}
		});

		this
	}

	/// Perform the MoQ handshake as a client.
	pub async fn connect(mut session: web_transport::Session) -> Result<Self, Error> {
		let mut stream = Stream::open(&mut session, message::Stream::Session).await?;
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
	pub async fn accept(mut session: web_transport::Session) -> Result<Self, Error> {
		let mut stream = Stream::accept(&mut session).await?;
		let kind = stream.reader.decode().await?;

		if kind != message::Stream::Session {
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
			let mut subscriber = subscriber.clone();

			spawn(async move {
				match stream.decode().await {
					Ok(message::StreamUni::Group) => subscriber.recv_group(stream).await,
					Err(err) => stream.close(err),
				};
			});
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
			message::Stream::Session => Err(Error::UnexpectedStream(kind)),
			message::Stream::Announce => publisher.recv_announce(stream).await,
			message::Stream::Subscribe => publisher.recv_subscribe(stream).await,
			message::Stream::Fetch => publisher.recv_fetch(stream).await,
			message::Stream::Info => publisher.recv_info(stream).await,
		}
	}

	/// Publish a broadcast.
	pub fn publish(&mut self, broadcast: BroadcastConsumer) -> Result<(), Error> {
		self.publisher.publish(broadcast)
	}

	/// Subscribe to a broadcast.
	///
	/// NOTE: Nothing flows over the network until an individual track is requested.
	pub fn subscribe<T: Into<Broadcast>>(&self, broadcast: T) -> BroadcastConsumer {
		self.subscriber.broadcast(broadcast.into())
	}

	/// Discover any broadcasts.
	pub fn announced(&self) -> AnnouncedConsumer {
		self.announced_prefix(Path::default())
	}

	/// Discover any broadcasts matching a prefix.
	pub fn announced_prefix(&self, prefix: Path) -> AnnouncedConsumer {
		self.subscriber.broadcasts(prefix)
	}

	pub fn close(mut self, err: Error) {
		self.webtransport.close(err.to_code(), &err.to_string());
	}

	pub async fn closed(&self) -> Error {
		self.webtransport.closed().await.into()
	}
}
