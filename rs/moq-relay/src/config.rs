use std::path::PathBuf;

use clap::Parser;
use figment::{
	providers::{Env, Format, Serialized, Toml},
	Figment,
};
use serde::{Deserialize, Serialize};

#[derive(Parser, Clone, Deserialize, Serialize, Debug)]
pub struct Config {
	/// Path to a TOML config file
	pub config: Option<PathBuf>,

	/// Listen on this address, both TCP and UDP.
	//#[bpaf(fallback("[::]:443"))]
	pub bind: String,

	/// The TLS configuration.
	#[clap(flatten)]
	pub tls: moq_native::tls::Args,

	/// Log configuration.
	#[clap(flatten)]
	pub log: moq_native::log::Args,

	/// Cluster configuration.
	#[clap(flatten)]
	pub cluster: ClusterConfig,

	/// Authentication configuration.
	#[clap(flatten)]
	pub auth: AuthConfig,
}

impl Config {
	pub fn load() -> anyhow::Result<Self> {
		let args = Config::parse();

		let mut figment = Figment::new().merge(Serialized::defaults(&args));
		if let Some(config) = args.config {
			figment = figment.merge(Toml::file(config));
		}

		let config: Config = figment.merge(Env::prefixed("MOQ_")).extract()?;
		config.log.init();

		tracing::debug!(?config, "loaded config");
		Ok(config)
	}
}

#[derive(Clone, Parser, Serialize, Deserialize, Debug, Default)]
pub struct ClusterConfig {
	/// Announce our tracks and discover other origins via this server.
	/// If not provided, then clustering is disabled.
	///
	/// Peers will connect to use via this hostname.
	#[arg(long = "cluster-root")]
	pub root: Option<String>,

	/// Our unique name which we advertise to other origins.
	/// If not provided, then we are a read-only member of the cluster.
	///
	/// Peers will connect to use via this hostname.
	#[arg(long = "cluster-node")]
	pub node: Option<String>,
}

#[derive(Clone, Parser, Serialize, Deserialize, Debug, Default)]
pub struct AuthConfig {}
