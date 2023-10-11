use clap::Parser;

use moq_api::{Server, ServerConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	tracing_subscriber::fmt::init();

	let config = ServerConfig::parse();
	let server = Server::new(config);
	server.run().await
}
