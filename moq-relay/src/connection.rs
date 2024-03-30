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

		if let Some(publisher) = publisher {
			tasks.push(Self::serve_publisher(publisher, self.origin.clone()).boxed());
		}

		if let Some(subscriber) = subscriber {
			tasks.push(Self::serve_subscriber(subscriber, self.origin).boxed());
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

		if let Some(publisher) = publisher {
			tasks.push(Self::serve_publisher(publisher, self.origin.clone()).boxed());
		}

		if let Some(subscriber) = subscriber {
			tasks.push(Self::serve_subscriber(subscriber, self.origin).boxed());
		}

		// Return the first error
		tasks.select_next_some().await?;

		Ok(())
	}

	async fn serve_publisher<S: webtransport_generic::Session>(
		mut publisher: Publisher<S>,
		origin: Origin,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				subscribe = publisher.subscribed() => {
					let origin = origin.clone();
					tasks.push(async move {
						let info = subscribe.info.clone();

						if let Err(err) = Self::serve_subscribe(origin.clone(), subscribe).await {
							log::warn!("failed serving subscribe: info={:?} err={:?}", info, err)

						}
					});
				},
				_= tasks.select_next_some() => {},
			};
		}
	}

	async fn serve_subscribe<S: webtransport_generic::Session>(
		origin: Origin,
		subscribe: Subscribed<S>,
	) -> Result<(), RelayError> {
		log::info!("serving subscribe: info={:?}", subscribe.info,);

		let track = origin.subscribe(&subscribe.namespace, &subscribe.name)?;
		subscribe.serve(track).await?;

		Ok(())
	}

	async fn serve_subscriber<S: webtransport_generic::Session>(
		mut subscriber: Subscriber<S>,
		origin: Origin,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				announce = subscriber.announced() => {
					let info = announce.info.clone();
					let subscriber = subscriber.clone();
					let origin = origin.clone();

					tasks.push(async move {
						if let Err(err) = Self::serve_announce(subscriber, origin, announce).await {
							log::warn!("failed serving announce: info={:?} err={:?}", info, err)
						}
					});
				},
				_ = tasks.select_next_some() => {},
			};
		}
	}

	async fn serve_announce<S: webtransport_generic::Session>(
		subscriber: Subscriber<S>,
		mut origin: Origin,
		announce: Announced<S>,
	) -> Result<(), RelayError> {
		log::info!("serving announce: info={:?}", announce.info);

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
					let mut subscriber = subscriber.clone();

					tasks.push(async move {
						let info = track.info.clone();
						log::info!("local subscribe: track={:?}", track);

						match subscriber.subscribe(track) {
							Ok(subscribe) => if let Err(err) = subscribe.run().await {
								log::warn!("local subscribe done: track={:?} err={:?}", info, err);
							},
							Err(err) => {
								log::warn!("local subscribe failed: track={:?} err={:?}", info, err);
							},
						};
					});
				},
				_ = tasks.select_next_some() => {}
			}
		}
	}
}
