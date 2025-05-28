mod client;
mod server;

use std::path::PathBuf;

use client::*;
use server::*;

use clap::{Parser, Subcommand};
use url::Url;

#[derive(Parser, Clone)]
pub struct Cli {
	#[command(flatten)]
	log: moq_native::Log,

	#[command(subcommand)]
	command: Command,
}

#[derive(Subcommand, Clone)]
pub enum Command {
	Serve {
		#[command(flatten)]
		config: moq_native::ServerConfig,

		/// Optionally serve static files from the given directory.
		#[arg(long)]
		dir: Option<PathBuf>,
	},
	Publish {
		/// The MoQ client configuration.
		#[command(flatten)]
		config: moq_native::ClientConfig,

		/// The URL of the MoQ server.
		///
		/// The URL must start with `https://` or `http://`.
		/// - If `http` is used, a HTTP fetch to "/certificate.sha256" is first made to get the TLS certificiate fingerprint (insecure).
		///   The URL is then upgraded to `https`.
		///
		/// - If `https` is used, then A WebTransport connection is made via QUIC to the provided host/port.
		///   The path is used to identify the broadcast, with the rest of the URL (ex. query/fragment) currently ignored.
		url: Url,
	},
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();
	cli.log.init();

	match cli.command {
		Command::Serve { config, dir } => server(config, dir, &mut tokio::io::stdin()).await,
		Command::Publish { config, url } => client(config, url, &mut tokio::io::stdin()).await,
	}
}
