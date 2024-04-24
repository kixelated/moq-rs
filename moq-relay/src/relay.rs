use std::net;

use anyhow::Context;

use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_native::quic;
use url::Url;

use crate::{Api, Consumer, Locals, Producer, Remotes, RemotesConsumer, RemotesProducer, Session};

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
	api: Option<Api>,
	remotes: Option<(RemotesProducer, RemotesConsumer)>,
}

impl Relay {
	// Create a QUIC endpoint that can be used for both clients and servers.
	pub fn new(config: RelayConfig) -> anyhow::Result<Self> {
		let quic = quic::Endpoint::new(quic::Config {
			bind: config.bind,
			tls: config.tls,
		})?;

		let api = if let (Some(url), Some(node)) = (config.api, config.node) {
			log::info!("using moq-api: url={} node={}", url, node);
			Some(Api::new(url, node))
		} else {
			None
		};

		let locals = Locals::new();

		let remotes = api.clone().map(|api| {
			Remotes {
				api,
				quic: quic.client.clone(),
			}
			.produce()
		});

		Ok(Self {
			quic,
			announce: config.announce,
			api,
			locals,
			remotes,
		})
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let mut tasks = FuturesUnordered::new();

		let remotes = self.remotes.map(|(producer, consumer)| {
			tasks.push(async move { producer.run().await.context("remotes producer failed") }.boxed_local());
			consumer
		});

		let forward = if let Some(url) = &self.announce {
			log::info!("forwarding announces to {}", url);
			let session = self
				.quic
				.client
				.connect(url)
				.await
				.context("failed to establish forward connection")?;
			let (session, publisher, subscriber) = moq_transport::session::Session::connect(session)
				.await
				.context("failed to establish forward session")?;

			// Create a normal looking session, except we never forward or register announces.
			let session = Session {
				session,
				producer: Some(Producer::new(publisher, self.locals.clone(), remotes.clone())),
				consumer: Some(Consumer::new(subscriber, self.locals.clone(), None, None)),
			};

			let forward = session.producer.clone();

			tasks.push(async move { session.run().await.context("forwarding failed") }.boxed_local());

			forward
		} else {
			None
		};

		let mut server = self.quic.server.context("missing TLS certificate")?;
		log::info!("listening on {}", server.local_addr()?);

		loop {
			tokio::select! {
				res = server.accept() => {
					let conn = res.context("failed to accept QUIC connection")?;

					let locals = self.locals.clone();
					let remotes = remotes.clone();
					let forward = forward.clone();
					let api = self.api.clone();

					tasks.push(async move {
						let (session, publisher, subscriber) = match moq_transport::session::Session::accept(conn).await {
							Ok(session) => session,
							Err(err) => {
								log::warn!("failed to accept MoQ session: {}", err);
								return Ok(());
							}
						};

						let session = Session {
							session,
							producer: publisher.map(|publisher| Producer::new(publisher, locals.clone(), remotes)),
							consumer: subscriber.map(|subscriber| Consumer::new(subscriber, locals, api, forward)),
						};

						if let Err(err) = session.run().await {
							log::warn!("failed to run MoQ session: {}", err);
						}

						Ok(())
					}.boxed_local());
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?,
			}
		}
	}
}
