use clap::Parser;

mod consumer;
mod local;
//mod origin;
mod producer;
mod relay;
//mod remote;
mod session;
mod web;

pub use consumer::*;
pub use local::*;
//pub use origin::*;
pub use producer::*;
pub use relay::*;
//pub use remote::*;
pub use session::*;
pub use web::*;

use std::net;
use url::Url;

#[derive(Parser, Clone)]
pub struct Cli {
	/// Listen on this address
	#[arg(long, default_value = "[::]:443")]
	pub bind: net::SocketAddr,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Args,

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
	moq_native::log::init();

	let cli = Cli::parse();
	let tls = cli.tls.load()?;

	if tls.server.is_none() {
		anyhow::bail!("missing TLS certificates");
	}

	// Create a QUIC server for media.
	let relay = Relay::new(RelayConfig {
		tls: tls.clone(),
		bind: cli.bind,
		host: cli.node,
		announce: cli.announce,
	});

	if cli.dev {
		// Create a web server too.
		// Currently this only contains the certificate fingerprint (for development only).
		let web = Web::new(WebConfig { bind: cli.bind, tls });

		tokio::spawn(async move {
			web.run().await.expect("failed to run web server");
		});
	}

	relay.run().await
}
