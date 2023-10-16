use anyhow::Context;
use clap::Parser;

mod config;
mod error;
mod origin;
mod quic;
mod session;
mod tls;
mod web;

pub use config::*;
pub use error::*;
pub use origin::*;
pub use quic::*;
pub use session::*;
pub use tls::*;
pub use web::*;

#[tokio::main]
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
	let quic = Quic::new(config.clone(), tls.clone())
		.await
		.context("failed to create server")?;

	// Create the web server if the --dev flag was set.
	// This is currently only useful in local development so it's not enabled by default.
	if config.dev {
		let web = Web::new(config, tls);

		// Unfortunately we can't use preconditions because Tokio still executes the branch; just ignore the result
		tokio::select! {
			res = quic.serve() => res.context("failed to run quic server"),
			res = web.serve() => res.context("failed to run web server"),
		}
	} else {
		quic.serve().await.context("failed to run quic server")
	}
}
