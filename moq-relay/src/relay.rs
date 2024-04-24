use std::net;

use anyhow::Context;

use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_native::quic;
use moq_transport::session::Publisher;
use url::Url;

use crate::{Locals, Remotes, RemotesConsumer, RemotesProducer, Session};

pub struct RelayConfig {
	/// Listen on this address
	pub bind: net::SocketAddr,

	/// The TLS configuration.
	pub tls: moq_native::tls::Config,

	/// Forward all announcements to the (optional) URL.
	pub announce: Option<Url>,

	/// Connect to the HTTP moq-api at this URL.
	pub api: Option<Url>,

	/// Our hostname which we advertise to other origins.
	/// We use QUIC, so the certificate must be valid for this address.
	pub node: Option<Url>,
}

pub struct Relay {
	quic: quic::Endpoint,
	announce: Option<Url>,
	locals: Locals,
	remotes: Option<(RemotesProducer, RemotesConsumer)>,
}

impl Relay {
	// Create a QUIC endpoint that can be used for both clients and servers.
	pub fn new(config: RelayConfig) -> anyhow::Result<Self> {
		let quic = quic::Endpoint::new(quic::Config {
			bind: config.bind,
			tls: config.tls,
		})?;

		let api = config.api.map(|url| {
			log::info!("using moq-api: url={}", url);
			moq_api::Client::new(url)
		});

		let node = config.node.map(|node| {
			log::info!("advertising origin: url={}", node);
			node
		});

		let locals = Locals::new(api.clone(), node);

		let remotes = api.map(|api| {
			Remotes {
				api,
				quic: quic.client.clone(),
			}
			.produce()
		});

		Ok(Self {
			quic,
			announce: config.announce,
			locals,
			remotes,
		})
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let mut tasks = FuturesUnordered::new();

		let forward = if let Some(url) = &self.announce {
			log::info!("forwarding announces to {}", url);
			let session = self.quic.client.connect(url).await?;
			let (session, publisher) = Publisher::connect(session).await?;
			tasks.push(async move { session.run().await.context("forwarding announces failed") }.boxed_local());

			Some(publisher)
		} else {
			None
		};

		let remotes = self.remotes.map(|(producer, consumer)| {
			tasks.push(producer.run().boxed_local());
			consumer
		});

		let mut server = self.quic.server.context("missing TLS certificate")?;
		log::info!("listening on {}", server.local_addr()?);

		loop {
			tokio::select! {
				res = server.accept() => {
					let conn = res.context("failed to accept QUIC connection")?;
					let session = Session::new(conn, self.locals.clone(), remotes.clone(), forward.clone());

					tasks.push(async move {
						if let Err(err) = session.run().await {
							log::warn!("connection terminated: {}", err);
						}

						Ok(())
					}.boxed_local());
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?,
			}
		}
	}
}
