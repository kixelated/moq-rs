use std::net;
use webtransport_quinn;
use http;

use anyhow::Context;

pub struct ClientConfig {
	pub addr: net::SocketAddr,
    pub uri: http::uri::Uri,
}

pub struct Client {
    client: quinn::Endpoint,
    config: ClientConfig,
}

impl Client {
   pub async fn new(config: ClientConfig) -> anyhow::Result<Self> {
       let endpoint = quinn::Endpoint::client(config.addr)?;
       Ok(Client { client: endpoint, config })
   }

   pub async fn run(self) -> anyhow::Result<()> {
       let session = webtransport_quinn::connect(&self.client, &self.config.uri).await.context("failed to create session")?;
       let session = moq_transport_quinn::connect(session, moq_transport::Role::Publisher).await.context("failed to create session")?;
       panic!("got a session - now what?");
   }
}
