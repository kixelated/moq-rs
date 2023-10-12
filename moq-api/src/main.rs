use clap::Parser;

mod server;
use moq_api::ApiError;
use server::{Server, ServerConfig};

#[tokio::main]
async fn main() -> Result<(), ApiError> {
	env_logger::init();

	let config = ServerConfig::parse();
	let server = Server::new(config);
	server.run().await
}
