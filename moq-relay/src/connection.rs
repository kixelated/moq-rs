use anyhow::Context;

use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_transport::{
	serve::ServeError,
	session::{Announced, Publisher, SessionError, Subscribed, Subscriber},
};

use crate::{LocalsConsumer, LocalsProducer, RelayError, RemotesConsumer};

#[derive(Clone)]
pub struct Connection {
	locals: (LocalsProducer, LocalsConsumer),
	remotes: Option<RemotesConsumer>,
}

impl Connection {
	pub fn new(locals: (LocalsProducer, LocalsConsumer), remotes: Option<RemotesConsumer>) -> Self {
		Self { locals, remotes }
	}

	pub async fn run(self, mut conn: quinn::Connecting) -> anyhow::Result<()> {
		let handshake = conn
			.handshake_data()
			.await?
			.downcast::<quinn::crypto::rustls::HandshakeData>()
			.unwrap();

		let alpn = handshake.protocol.context("missing ALPN")?;
		let alpn = String::from_utf8_lossy(&alpn);
		let server_name = handshake.server_name.unwrap_or_default();

		log::debug!(
			"received QUIC handshake: ip={} alpn={} server={}",
			conn.remote_address(),
			alpn,
			server_name,
		);

		// Wait for the QUIC connection to be established.
		let conn = conn.await.context("failed to establish QUIC connection")?;

		log::debug!(
			"established QUIC connection: id={} ip={} alpn={} server={}",
			conn.stable_id(),
			conn.remote_address(),
			alpn,
			server_name,
		);

		match alpn.as_bytes() {
			webtransport_quinn::ALPN => self.serve_webtransport(conn).await?,
			moq_transport::setup::ALPN => self.serve_quic(conn).await?,
			_ => anyhow::bail!("unsupported ALPN: {}", alpn),
		}

		Ok(())
	}

	async fn serve_webtransport(self, conn: quinn::Connection) -> anyhow::Result<()> {
		// Wait for the CONNECT request.
		let request = webtransport_quinn::accept(conn)
			.await
			.context("failed to receive WebTransport request")?;

		// Accept the CONNECT request.
		let session = request
			.ok()
			.await
			.context("failed to respond to WebTransport request")?;

		let (session, publisher, subscriber) = moq_transport::Session::accept(session).await?;

		let mut tasks = FuturesUnordered::new();
		tasks.push(session.run().boxed());

		if let Some(remote) = publisher {
			tasks.push(Self::serve_subscriber(self.clone(), remote).boxed());
		}

		if let Some(remote) = subscriber {
			tasks.push(Self::serve_publisher(self.clone(), remote).boxed());
		}

		// Return the first error
		tasks.select_next_some().await?;

		Ok(())
	}

	async fn serve_quic(self, conn: quinn::Connection) -> anyhow::Result<()> {
		let session: quictransport_quinn::Session = conn.into();

		let (session, publisher, subscriber) = moq_transport::Session::accept(session).await?;

		let mut tasks = FuturesUnordered::new();
		tasks.push(session.run().boxed());

		if let Some(remote) = publisher {
			tasks.push(Self::serve_subscriber(self.clone(), remote).boxed());
		}

		if let Some(remote) = subscriber {
			tasks.push(Self::serve_publisher(self.clone(), remote).boxed());
		}

		// Return the first error
		tasks.select_next_some().await?;

		Ok(())
	}

	async fn serve_subscriber<S: webtransport_generic::Session>(
		self,
		mut remote: Publisher<S>,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				subscribe = remote.subscribed() => {
					let conn = self.clone();

					tasks.push(async move {
						let info = subscribe.info.clone();
						log::info!("serving subscribe: {:?}", info);

						if let Err(err) = conn.serve_subscribe(subscribe).await {
							log::warn!("failed serving subscribe: {:?}, error: {}", info, err)
						}
					})
				},
				_= tasks.next(), if !tasks.is_empty() => {},
			};
		}
	}

	async fn serve_subscribe<S: webtransport_generic::Session>(
		self,
		subscribe: Subscribed<S>,
	) -> Result<(), RelayError> {
		if let Some(local) = self.locals.1.route(&subscribe.namespace) {
			log::debug!("using local announce: {:?}", local.info);
			if let Some(track) = local.subscribe(&subscribe.name)? {
				log::info!("serving from local: {:?}", track.info);
				// NOTE: Depends on drop(track) being called afterwards
				return Ok(subscribe.serve(track.reader).await?);
			}
		}

		if let Some(remotes) = &self.remotes {
			if let Some(remote) = remotes.route(&subscribe.namespace).await? {
				log::debug!("using remote announce: {:?}", remote.info);
				if let Some(track) = remote.subscribe(&subscribe.namespace, &subscribe.name)? {
					log::info!("serving from remote: {:?} {:?}", remote.info, track.info);

					// NOTE: Depends on drop(track) being called afterwards
					return Ok(subscribe.serve(track.reader).await?);
				}
			}
		}

		Err(ServeError::NotFound.into())
	}

	async fn serve_publisher<S: webtransport_generic::Session>(
		self,
		mut remote: Subscriber<S>,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				announce = remote.announced() => {
					let remote = remote.clone();
					let conn = self.clone();

					tasks.push(async move {
						let info = announce.info.clone();
						log::info!("serving announce: {:?}", info);

						if let Err(err) = conn.serve_announce(remote, announce).await {
							log::warn!("failed serving announce: {:?}, error: {}", info, err)
						}
					});
				},
				_ = tasks.next(), if !tasks.is_empty() => {},
			};
		}
	}

	async fn serve_announce<S: webtransport_generic::Session>(
		mut self,
		remote: Subscriber<S>,
		mut announce: Announced<S>,
	) -> Result<(), RelayError> {
		let mut publisher = match self.locals.0.announce(&announce.namespace).await {
			Ok(publisher) => {
				announce.ok()?;
				publisher
			}
			Err(err) => {
				// TODO use better error codes
				announce.close(err.clone().into())?;
				return Err(err);
			}
		};

		let mut tasks = FuturesUnordered::new();

		let mut done = None;

		loop {
			tokio::select! {
				// If the announce is closed, return the error
				res = announce.closed(), if done.is_none() => done = Some(res),

				// Wait for the next subscriber and serve the track.
				res = publisher.requested(), if done.is_none() => {
					let track = match res? {
						Some(track) => track,
						None => {
							done = Some(Ok(()));
							continue
						},
					};

					let mut subscriber = remote.clone();

					tasks.push(async move {
						let info = track.info.clone();
						log::info!("relaying track: track={:?}", info);

						let res = match subscriber.subscribe(track) {
							Ok(subscribe) => subscribe.closed().await,
							Err(err) => Err(err),
						};

						if let Err(err) = res {
							log::warn!("failed serving track: {:?}, error: {}", info, err)
						}
					});
				},
				_ = tasks.next(), if !tasks.is_empty() => {}

				// Done must be set and there are no tasks left
				else => return Ok(done.unwrap()?),
			}
		}
	}
}
