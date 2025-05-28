use serde::{Deserialize, Serialize};
use serde_with::DisplayFromStr;
use tracing::level_filters::LevelFilter;
use tracing::Level;
use tracing_subscriber::EnvFilter;

#[serde_with::serde_as]
#[derive(Clone, clap::Parser, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields, default)]
pub struct Log {
	/// The level filter to use.
	#[serde_as(as = "DisplayFromStr")]
	#[arg(id = "log-level", long = "log-level", default_value = "info")]
	pub level: Level,
}

impl Default for Log {
	fn default() -> Self {
		Self { level: Level::INFO }
	}
}

impl Log {
	pub fn level(&self) -> LevelFilter {
		LevelFilter::from_level(self.level)
	}

	pub fn init(&self) {
		let filter = EnvFilter::builder()
			.with_default_directive(self.level().into()) // Default to our -q/-v args
			.from_env_lossy() // Allow overriding with RUST_LOG
			.add_directive("h2=warn".parse().unwrap())
			.add_directive("quinn=info".parse().unwrap())
			.add_directive("tracing::span=off".parse().unwrap())
			.add_directive("tracing::span::active=off".parse().unwrap());

		let logger = tracing_subscriber::FmtSubscriber::builder()
			.with_writer(std::io::stderr)
			.with_env_filter(filter)
			.finish();

		tracing::subscriber::set_global_default(logger).unwrap();
	}
}
