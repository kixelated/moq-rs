use anyhow::Context;
use clap::Parser;

mod local;
mod relay;
mod remote;
mod session;
mod web;

pub use local::*;
pub use relay::*;
pub use remote::*;
pub use session::*;
pub use web::*;

use std::net;
use url::Url;

#[derive(Parser, Clone)]
pub struct Cli {
	/// Listen on this address
	#[arg(long, default_value = "[::]:4443")]
	pub bind: net::SocketAddr,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Cli,

	/// Forward all announces to the provided server for authentication/routing.
	/// If not provided, the relay accepts every unique announce.
	#[arg(long)]
	pub announce: Option<Url>,

	/// The URL of the moq-api server in order to run a cluster.
	/// Must be used in conjunction with --node to advertise the origin
	#[arg(long)]
	pub api: Option<Url>,

	/// The hostname that we advertise to other origins.
	/// The provided certificate must be valid for this address.
	#[arg(long)]
	pub node: Option<Url>,

	/// Enable development mode.
	/// Currently, this only listens on HTTPS and serves /fingerprint, for self-signed certificates
	#[arg(long, action)]
	pub dev: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();

	// Disable tracing so we don't get a bunch of Quinn spam.
	let tracer = tracing_subscriber::FmtSubscriber::builder()
		.with_max_level(tracing::Level::WARN)
		.finish();
	tracing::subscriber::set_global_default(tracer).unwrap();

	let cli = Cli::parse();
	let tls = cli.tls.load()?;

	if tls.server.is_none() {
		anyhow::bail!("missing TLS certificates");
	}

	// Create a QUIC server for media.
	let relay = Relay::new(RelayConfig {
		tls: tls.clone(),
		bind: cli.bind,
		node: cli.node,
		api: cli.api,
		announce: cli.announce,
	})?;

	// Create the web server if the --dev flag was set.
	// This is currently only useful in local development so it's not enabled by default.
	if cli.dev {
		let web = Web::new(WebConfig { bind: cli.bind, tls });

		// Unfortunately we can't use preconditions because Tokio still executes the branch; just ignore the result
		tokio::select! {
			res = relay.run() => res.context("failed to run quic server"),
			res = web.serve() => res.context("failed to run web server"),
		}
	} else {
		relay.run().await.context("failed to run quic server")
	}
}
