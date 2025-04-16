mod client;
mod config;
mod server;

use client::*;
use config::*;
use server::*;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let config = Config::parse();
	config.log.init();

	match config.command.clone() {
		Command::Serve(server) => Server::new(config, server).await?.run(&mut tokio::io::stdin()).await,
		Command::Publish(client) => BroadcastClient::new(config, client).run(&mut tokio::io::stdin()).await,
	}
}
