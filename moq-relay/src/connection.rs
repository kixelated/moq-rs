use anyhow::Context;

use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_transport::{
	serve::ServeError,
	session::{Announced, Publisher, SessionError, Subscriber},
};

use crate::Origin;

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
		tasks.next().await.unwrap()?;

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
		tasks.next().await.unwrap()?;

		Ok(())
	}

	async fn serve_publisher<S: webtransport_generic::Session>(
		mut publisher: Publisher<S>,
		origin: Origin,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = tasks.next(), if !tasks.is_empty() => {
					if let Err(err) = res.unwrap() {
						log::info!("failed serving subscribe: err={}", err)
					}
				},
				subscribe = publisher.subscribed() => {
					log::info!("serving subscribe: namespace={} name={}", subscribe.namespace(), subscribe.name());

					let track = origin.subscribe(subscribe.namespace(), subscribe.name())?;
					tasks.push(subscribe.serve(track));
				}
			};
		}
	}

	async fn serve_subscriber<S: webtransport_generic::Session>(
		mut subscriber: Subscriber<S>,
		origin: Origin,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = tasks.next(), if !tasks.is_empty() => {
					if let Err(err) = res.unwrap() {
						log::info!("failed serving announce: err={}", err)
					}
				},
				announce = subscriber.announced() => {
					tasks.push(Self::serve_announce(subscriber.clone(), origin.clone(), announce));
				}
			};
		}
	}

	async fn serve_announce<S: webtransport_generic::Session>(
		mut subscriber: Subscriber<S>,
		origin: Origin,
		announce: Announced<S>,
	) -> Result<(), ServeError> {
		log::info!("serving announce: namespace={}", announce.namespace());

		let mut publisher = match origin.announce(announce.namespace()) {
			Ok(publisher) => publisher,
			Err(err) => {
				announce.close(err.clone())?;
				return Err(err);
			}
		};

		let mut tasks = FuturesUnordered::new();

		// Send ANNOUNCE_OK and wait for any UNANNOUNCE
		let mut announce = announce.serve().boxed();

		loop {
			tokio::select! {
				// If the announce is closed, return the error
				res = &mut announce => return res,

				// Wait for the next subscriber and serve the track.
				res = publisher.requested() => {
					let track = res?.ok_or(ServeError::Done)?;
					log::info!("track requested: name={}", track.name);
					let subscribe = subscriber.subscribe(track)?;
					tasks.push(subscribe.serve());
				},
				res = tasks.next(), if !tasks.is_empty() => {
					log::info!("done serving subscribe");
					if let Err(err) = res.unwrap() {
						log::warn!("failed serving subscribe: err={}", err)
					}
				},
			}
		}
	}
}
