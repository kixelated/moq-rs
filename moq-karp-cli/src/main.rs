mod client;
mod config;
mod fingerprint;
mod server;

use client::*;
use config::*;
use fingerprint::*;
use server::*;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let config = Config::parse();
	config.log.init();

	match config.command.clone() {
		Command::Serve { path } => BroadcastServer::new(config, path, tokio::io::stdin()).run().await,
		Command::Publish { url } => BroadcastClient::new(config, url, tokio::io::stdin()).run().await,
	}
}
