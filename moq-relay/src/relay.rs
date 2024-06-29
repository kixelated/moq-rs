use std::net;

use anyhow::Context;

use futures::{stream::FuturesUnordered, StreamExt};
use moq_native::quic;
use url::Url;

use crate::{Consumer, Locals, Producer, Session};

pub struct RelayConfig {
	/// Listen on this address
	pub bind: net::SocketAddr,

	/// The TLS configuration.
	pub tls: moq_native::tls::Config,

	/// Forward announcements to the (optional) URL.
	/// If not provided, then we can't discover other origins.
	pub announce: Option<Url>,

	/// Our hostname which we advertise to other origins.
	/// We use QUIC, so the certificate must be valid for this address.
	/// If not provided, we don't advertise our origin.
	pub host: Option<String>,
}

pub struct Relay {
	config: RelayConfig,
}

impl Relay {
	// Create a QUIC endpoint that can be used for both clients and servers.
	pub fn new(config: RelayConfig) -> Self {
		Self { config }
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let mut tasks = FuturesUnordered::new();

		let quic = quic::Endpoint::new(quic::Config {
			bind: self.config.bind,
			tls: self.config.tls,
		})?;

		/*
		let root = if let Some(url) = self.config.announce {
			tracing::info!("forwarding announces to {}", url);
			let conn = quic
				.client
				.connect(&url)
				.await
				.context("failed to connect to announce server")?;

			let (session, publisher, subscriber) = moq_transfork::Session::connect(conn)
				.await
				.context("failed to establish announce session")?;

			tasks.push(session.run().boxed());
			Some((publisher, subscriber))
		} else {
			None
		};
		*/

		let locals = Locals::new(/*self.config.host*/);
		// let remotes = Remotes::new();

		let mut server = quic.server.context("missing TLS certificate")?;
		tracing::info!("listening on {}", server.local_addr()?);

		loop {
			tokio::select! {
				res = server.accept() => {
					let conn = res.context("failed to accept QUIC connection")?;

					let locals = locals.clone();
					//let remotes = remotes.clone();
					//let root = root.clone();

					tasks.push(async move {
						let (session, publisher, subscriber) = moq_transfork::Session::accept(conn).await?;
						let session = Session {
							session,
							producer: publisher.map(|publisher| Producer::new(publisher, locals.clone())),
							consumer: subscriber.map(|subscriber| Consumer::new(subscriber, locals)),
						};

						session.run().await
					});
				},
				res = tasks.next(), if !tasks.is_empty() => {
					if let Err(err) = res.unwrap() {
						tracing::warn!("failed to run MoQ session: {}", err);
					}
				},
			}
		}
	}
}
