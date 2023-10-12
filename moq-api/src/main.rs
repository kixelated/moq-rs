use clap::Parser;

mod server;
use moq_api::ApiError;
use server::{Server, ServerConfig};

#[tokio::main]
async fn main() -> Result<(), ApiError> {
	tracing_subscriber::fmt::init();

	let config = ServerConfig::parse();
	let server = Server::new(config);
	server.run().await
}
