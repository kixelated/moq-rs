use std::net;

pub struct Client {
    client: quinn::Endpoint,
}

pub struct ClientConfig {
	pub addr: net::SocketAddr,
}

impl Client {
   pub fn new(config: ClientConfig) -> anyhow::Result<Self> {
       todo!()
   }

    pub async fn run(mut self) -> anyhow::Result<()> {
        todo!()
    }

    pub fn handle(conn: quinn::Connecting) -> anyhow::Result<()> {
        todo!()
    }
}
