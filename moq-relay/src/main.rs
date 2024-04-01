use anyhow::Context;
use clap::Parser;

mod config;
mod connection;
mod error;
mod local;
mod relay;
mod remote;
mod tls;
mod web;

pub use config::*;
pub use connection::*;
pub use error::*;
pub use local::*;
pub use relay::*;
pub use remote::*;
pub use tls::*;
pub use web::*;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
	env_logger::init();

	// Disable tracing so we don't get a bunch of Quinn spam.
	let tracer = tracing_subscriber::FmtSubscriber::builder()
		.with_max_level(tracing::Level::WARN)
		.finish();
	tracing::subscriber::set_global_default(tracer).unwrap();

	let config = Config::parse();
	let tls = Tls::load(&config)?;

	// Create a QUIC server for media.
	let relay = Relay::new(config.clone(), tls.clone())
		.await
		.context("failed to create server")?;

	// Create the web server if the --dev flag was set.
	// This is currently only useful in local development so it's not enabled by default.
	if config.dev {
		let web = Web::new(config, tls);

		// Unfortunately we can't use preconditions because Tokio still executes the branch; just ignore the result
		tokio::select! {
			res = relay.run() => res.context("failed to run quic server"),
			res = web.serve() => res.context("failed to run web server"),
		}
	} else {
		relay.run().await.context("failed to run quic server")
	}
}
