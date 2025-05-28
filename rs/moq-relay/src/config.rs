use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::{AuthConfig, ClusterConfig};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
	/// The QUIC/TLS configuration for the server.
	pub server: moq_native::ServerConfig,

	/// The QUIC/TLS configuration for the client. (clustering only)
	#[serde(default)]
	pub client: moq_native::ClientConfig,

	/// Log configuration.
	#[serde(default)]
	pub log: moq_native::Log,

	/// Cluster configuration.
	#[serde(default)]
	pub cluster: ClusterConfig,

	/// Authentication configuration.
	#[serde(default)]
	pub auth: AuthConfig,
}

impl Config {
	pub fn load() -> anyhow::Result<Self> {
		let path = std::env::args()
			.nth(1)
			.context("the only argument is a TOML configuration file")?;

		let config: Config = toml::from_str(&std::fs::read_to_string(path)?)?;
		config.log.init();

		Ok(config)
	}
}
