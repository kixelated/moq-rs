use crate::{message, AnnounceConsumer, BroadcastConsumer, Error, Origin};

use web_async::spawn;

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

/// A MoQ session, used to publish and/or subscribe to broadcasts.
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
				tracing::info!(?err, "session terminated");
				session.close(err.to_code(), &err.to_string());
			}
		});

		this
	}

	/// Perform the MoQ handshake as a client.
	pub async fn connect<T: Into<web_transport::Session>>(session: T) -> Result<Self, Error> {
		let mut session = session.into();
		let mut stream = Stream::open(&mut session, message::ControlType::Session).await?;
		Self::connect_setup(&mut stream).await?;
		Ok(Self::new(session, stream))
	}

	async fn connect_setup(setup: &mut Stream) -> Result<(), Error> {
		let client = message::ClientSetup {
			versions: [message::Version::CURRENT].into(),
			extensions: Default::default(),
		};

		setup.writer.encode(&client).await?;
		let server: message::ServerSetup = setup.reader.decode().await?;

		tracing::debug!(version = ?server.version, "connected");

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

		Self::accept_setup(&mut stream).await?;
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

		tracing::debug!(version = ?server.version, "connected");

		Ok(())
	}

	async fn run_session(mut stream: Stream) -> Result<(), Error> {
		while let Some(_info) = stream.reader.decode_maybe::<message::SubscribeOk>().await? {}
		Err(Error::Cancel)
	}

	async fn run_uni(mut session: web_transport::Session, subscriber: Subscriber) -> Result<(), Error> {
		loop {
			let stream = Reader::accept(&mut session).await?;
			let subscriber = subscriber.clone();

			spawn(async move {
				Self::run_data(stream, subscriber).await.ok();
			});
		}
	}

	async fn run_data(mut stream: Reader, mut subscriber: Subscriber) -> Result<(), Error> {
		let kind = stream.decode().await?;

		let res = match kind {
			message::DataType::Group => subscriber.recv_group(&mut stream).await,
		};

		if let Err(err) = res {
			stream.abort(&err);
		}

		Ok(())
	}

	async fn run_bi(mut session: web_transport::Session, publisher: Publisher) -> Result<(), Error> {
		loop {
			let stream = Stream::accept(&mut session).await?;
			let publisher = publisher.clone();

			spawn(async move {
				Self::run_control(stream, publisher).await.ok();
			});
		}
	}

	async fn run_control(mut stream: Stream, mut publisher: Publisher) -> Result<(), Error> {
		let kind = stream.reader.decode().await?;

		let res = match kind {
			message::ControlType::Session => Err(Error::UnexpectedStream(kind)),
			message::ControlType::Announce => publisher.recv_announce(&mut stream).await,
			message::ControlType::Subscribe => publisher.recv_subscribe(&mut stream).await,
		};

		if let Err(err) = &res {
			stream.writer.abort(err);
		}

		res
	}

	/// Publish a broadcast, automatically announcing and serving it.
	pub fn publish<T: ToString>(&mut self, path: T, broadcast: BroadcastConsumer) {
		self.publisher.publish(path, broadcast);
	}

	/// Scope subscriptions to a broadcast, returning a handle that can request tracks.
	///
	/// No data flows over the network until [BroadcastConsumer::subscribe] is called.
	pub fn consume(&self, path: &str) -> BroadcastConsumer {
		self.subscriber.consume(path)
	}

	/// Discover any broadcasts published by the remote matching a prefix.
	///
	/// There will be an event each time a broadcast starts and later ends.
	/// The results contain the suffix only; you may need to re-apply the prefix.
	pub fn announced<S: ToString>(&self, prefix: S) -> AnnounceConsumer {
		self.subscriber.announced(prefix.to_string())
	}

	/// Close the underlying WebTransport session.
	pub fn close(mut self, err: Error) {
		self.webtransport.close(err.to_code(), &err.to_string());
	}

	/// Block until the WebTransport session is closed.
	pub async fn closed(&self) -> Error {
		self.webtransport.closed().await.into()
	}

	/// Publish all of our broadcasts to the given origin.
	///
	/// If an optional prefix is provided, the prefix will be applied when inserting into the origin.
	pub async fn publish_to(&mut self, mut origin: Origin, prefix: &str) {
		let mut announced = self.announced("");

		while let Some(suffix) = announced.active().await {
			let broadcast = self.consume(&suffix);

			// We can avoid a string copy if there's no prefix.
			match prefix {
				"" => origin.publish(suffix, broadcast),
				prefix => origin.publish(format!("{}{}", prefix, suffix), broadcast),
			};
		}
	}

	/// Serve all broadcasts from the given origin.
	///
	/// If the prefix is provided, then only broadcasts matching the (stripped) prefix are served.
	pub async fn consume_from(&mut self, origin: Origin, prefix: &str) {
		let mut remotes = origin.announced(prefix);

		while let Some(suffix) = remotes.active().await {
			match prefix {
				// If there's no prefix, we can avoid a string copy.
				"" => {
					if let Some(upstream) = origin.consume(&suffix) {
						// If the broadcast exists (it should...) then we serve it.
						self.publish(suffix, upstream);
					}
				}
				// We need to re-apply the prefix to get the full path on the origin.
				prefix => {
					let path = format!("{}{}", prefix, suffix);
					if let Some(upstream) = origin.consume(&path) {
						// If the broadcast exists (it should...) then we serve it.
						self.publish(path, upstream);
					}
				}
			};
		}
	}
}
