use clap::Parser;

//mod origin;
mod relay;
//mod remote;
mod connection;
mod origins;
mod web;

//pub use origin::*;
pub use relay::*;
//pub use remote::*;
pub use connection::*;
pub use origins::*;
pub use web::*;

use std::net;
use url::Url;

#[derive(Parser, Clone)]
pub struct Config {
	/// Listen on this address
	#[arg(long, default_value = "[::]:443")]
	pub bind: net::SocketAddr,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Args,

	/// Log configuration.
	#[command(flatten)]
	pub log: moq_native::log::Args,

	/// Forward all announces to the provided server for authentication/routing.
	/// If not provided, the relay accepts every unique announce.
	#[arg(long)]
	pub announce: Option<Url>,

	/// The hostname that we advertise to other origins.
	/// The provided certificate must be valid for this address.
	#[arg(long)]
	pub node: Option<String>,

	/// Enable development mode.
	/// This hosts a HTTPS web server via TCP to serve the fingerprint of the certificate.
	#[arg(long)]
	pub dev: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let config = Config::parse();
	config.log.init();

	let tls = config.tls.load()?;

	if tls.server.is_none() {
		anyhow::bail!("missing TLS certificates");
	}

	// Create a QUIC server for media.
	let relay = Relay::new(RelayConfig {
		tls: tls.clone(),
		bind: config.bind,
		host: config.node,
		announce: config.announce,
	});

	if config.dev {
		// Create a web server too.
		// Currently this only contains the certificate fingerprint (for development only).
		let web = Web::new(WebConfig { bind: config.bind, tls });

		tokio::spawn(async move {
			web.run().await.expect("failed to run web server");
		});
	}

	relay.run().await
}
