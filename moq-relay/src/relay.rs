use std::{sync::Arc, time};

use anyhow::Context;

use tokio::task::JoinSet;

use crate::{
	Config, Connection, Locals, LocalsConsumer, LocalsProducer, Remotes, RemotesConsumer, RemotesProducer, Tls,
};

pub struct Relay {
	quic: quinn::Endpoint,

	locals: (LocalsProducer, LocalsConsumer),
	remotes: Option<(RemotesProducer, RemotesConsumer)>,
}

impl Relay {
	// Create a QUIC endpoint that can be used for both clients and servers.
	pub async fn new(config: Config, tls: Tls) -> anyhow::Result<Self> {
		let mut client_config = tls.client.clone();
		let mut server_config = tls.server.clone();
		client_config.alpn_protocols = vec![webtransport_quinn::ALPN.to_vec(), moq_transport::setup::ALPN.to_vec()];
		server_config.alpn_protocols = vec![webtransport_quinn::ALPN.to_vec(), moq_transport::setup::ALPN.to_vec()];

		// Enable BBR congestion control
		// TODO validate the implementation
		let mut transport_config = quinn::TransportConfig::default();
		transport_config.max_idle_timeout(Some(time::Duration::from_secs(10).try_into().unwrap()));
		transport_config.keep_alive_interval(Some(time::Duration::from_secs(4))); // TODO make this smarter
		transport_config.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
		transport_config.mtu_discovery_config(None); // Disable MTU discovery
		let transport_config = Arc::new(transport_config);

		let mut client_config = quinn::ClientConfig::new(Arc::new(client_config));
		let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(server_config));
		server_config.transport_config(transport_config.clone());
		client_config.transport_config(transport_config);

		// There's a bit more boilerplate to make a generic endpoint.
		let runtime = quinn::default_runtime().context("no async runtime")?;
		let endpoint_config = quinn::EndpointConfig::default();
		let socket = std::net::UdpSocket::bind(config.listen).context("failed to bind UDP socket")?;

		// Create the generic QUIC endpoint.
		let mut quic = quinn::Endpoint::new(endpoint_config, Some(server_config), socket, runtime)
			.context("failed to create QUIC endpoint")?;
		quic.set_default_client_config(client_config);

		let api = config.api.map(|url| {
			log::info!("using moq-api: url={}", url);
			moq_api::Client::new(url)
		});

		let node = config.api_node.map(|node| {
			log::info!("advertising origin: url={}", node);
			node
		});

		let remotes = api.clone().map(|api| {
			Remotes {
				api,
				quic: quic.clone(),
			}
			.produce()
		});
		let locals = Locals { api, node }.produce();

		Ok(Self { quic, locals, remotes })
	}

	pub async fn run(self) -> anyhow::Result<()> {
		log::info!("listening on {}", self.quic.local_addr()?);

		let mut tasks = JoinSet::new();

		let remotes = self.remotes.map(|(producer, consumer)| {
			tasks.spawn(producer.run());
			consumer
		});

		loop {
			tokio::select! {
				res = self.quic.accept() => {
					let conn = res.context("failed to accept QUIC connection")?;
					let session = Connection::new(self.locals.clone(), remotes.clone());

					tasks.spawn(async move {
						if let Err(err) = session.run(conn).await {
							log::warn!("connection terminated: {:?}", err);
						}
						Ok(())
					});
				},
				res = tasks.join_next(), if !tasks.is_empty() => res.expect("no tasks").expect("task aborted")?,
			}
		}
	}
}
