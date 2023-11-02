use std::{sync::Arc, time};

use anyhow::Context;

use tokio::task::JoinSet;

use crate::{Config, Origin, Session, Tls};

pub struct Quic {
	quic: quinn::Endpoint,

	// The active connections.
	conns: JoinSet<anyhow::Result<()>>,

	// The map of active broadcasts by path.
	origin: Origin,
}

impl Quic {
	// Create a QUIC endpoint that can be used for both clients and servers.
	pub async fn new(config: Config, tls: Tls) -> anyhow::Result<Self> {
		let mut client_config = tls.client.clone();
		let mut server_config = tls.server.clone();
		client_config.alpn_protocols = vec![webtransport_quinn::ALPN.to_vec()];
		server_config.alpn_protocols = vec![webtransport_quinn::ALPN.to_vec()];

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

		if let Some(ref node) = config.api_node {
			log::info!("advertising origin: url={}", node);
		}

		let origin = Origin::new(api, config.api_node, quic.clone());
		let conns = JoinSet::new();

		Ok(Self { quic, origin, conns })
	}

	pub async fn serve(mut self) -> anyhow::Result<()> {
		log::info!("listening on {}", self.quic.local_addr()?);

		loop {
			tokio::select! {
				res = self.quic.accept() => {
					let conn = res.context("failed to accept QUIC connection")?;
					let mut session = Session::new(self.origin.clone());
					self.conns.spawn(async move { session.run(conn).await });
				},
				res = self.conns.join_next(), if !self.conns.is_empty() => {
					let res = res.expect("no tasks").expect("task aborted");
					if let Err(err) = res {
						log::warn!("connection terminated: {:?}", err);
					}
				},
			}
		}
	}
}
