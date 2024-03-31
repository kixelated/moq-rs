use anyhow::Context;

use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_transport::{
	serve::ServeError,
	session::{Announced, Publisher, SessionError, Subscribed, Subscriber},
};

use crate::{Origin, RelayError};

#[derive(Clone)]
pub struct Connection {
	origin: Origin,
}

impl Connection {
	pub fn new(origin: Origin) -> Self {
		Self { origin }
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
			tasks.push(Self::serve_subscriber(remote, self.origin.clone()).boxed());
		}

		if let Some(remote) = subscriber {
			tasks.push(Self::serve_publisher(remote, self.origin).boxed());
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
			tasks.push(Self::serve_subscriber(remote, self.origin.clone()).boxed());
		}

		if let Some(remote) = subscriber {
			tasks.push(Self::serve_publisher(remote, self.origin).boxed());
		}

		// Return the first error
		tasks.select_next_some().await?;

		Ok(())
	}

	async fn serve_subscriber<S: webtransport_generic::Session>(
		mut remote: Publisher<S>,
		origin: Origin,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				subscribe = remote.subscribed() => {
					let origin = origin.clone();

					tasks.push(async move {
						let info = subscribe.info.clone();

						match Self::serve_subscribe(origin, subscribe).await {
							Err(RelayError::Serve(err)) => log::info!("finished serving subscribe: info={:?} err={:?}", info, err),
							Err(err) => log::warn!("failed serving subscribe: info={:?} err={:?}", info, err),
							Ok(_) => log::info!("finished serving subscribe: info={:?}", info),
						}
					});
				},
				_= tasks.next(), if !tasks.is_empty() => {},
			};
		}
	}

	async fn serve_subscribe<S: webtransport_generic::Session>(
		origin: Origin,
		subscribe: Subscribed<S>,
	) -> Result<(), RelayError> {
		let track = match origin.subscribe(&subscribe.namespace, &subscribe.name) {
			Ok(track) => track,
			Err(err) => {
				subscribe.close(err.clone().into())?;
				return Err(err);
			}
		};

		subscribe.serve(track.reader).await?;

		Ok(())
	}

	async fn serve_publisher<S: webtransport_generic::Session>(
		mut remote: Subscriber<S>,
		origin: Origin,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				announce = remote.announced() => {
					let info = announce.info.clone();
					let remote = remote.clone();
					let origin = origin.clone();

					log::warn!("serving announce: info={:?}", info);

					tasks.push(async move {
						if let Err(err) = Self::serve_announce(remote, origin, announce).await {
							log::warn!("failed serving announce: info={:?} err={:?}", info, err)
						} else {
							log::info!("finished serving announce: info={:?}", info)
						}
					});
				},
				_ = tasks.next(), if !tasks.is_empty() => {},
			};
		}
	}

	async fn serve_announce<S: webtransport_generic::Session>(
		remote: Subscriber<S>,
		mut origin: Origin,
		announce: Announced<S>,
	) -> Result<(), RelayError> {
		let mut publisher = match origin.announce(&announce.namespace) {
			Ok(publisher) => publisher,
			Err(err) => {
				match &err {
					RelayError::Serve(err) => announce.close(err.clone()),
					_ => announce.close(ServeError::Closed(1)),
				}?;

				return Err(err.into());
			}
		};

		let mut tasks = FuturesUnordered::new();

		// Send ANNOUNCE_OK and wait for any UNANNOUNCE
		let mut announce = announce.serve().boxed();

		loop {
			tokio::select! {
				// If the announce is closed, return the error
				res = &mut announce => return Ok(res?),

				// Wait for the next subscriber and serve the track.
				res = publisher.requested() => {
					let track = res?.ok_or(ServeError::Done)?;
					let mut subscriber = remote.clone();

					tasks.push(async move {
						let info = track.info.clone();
						log::info!("relay: track={:?}", info);

						match subscriber.subscribe(track) {
							Ok(subscribe) => {
								let err = subscribe.closed().await;
								log::info!("relay finished: track={:?} err={:?}", info, err);
							},
							Err(err) => log::warn!("relay failed: track={:?} err={:?}", info, err),
						};
					});
				},
				_ = tasks.next(), if !tasks.is_empty() => {}
			}
		}
	}
}
