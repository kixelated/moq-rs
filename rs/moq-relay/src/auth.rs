use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use url::Url;

#[serde_with::serde_as]
#[derive(clap::Args, Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AuthConfig {
	/// The root key to use for all connections.
	///
	/// This is the fallback if a path does not exist in the `path` map below.
	/// If this is missing, then authentication is completely disabled, even if a path is configured below.
	#[serde(skip_serializing_if = "Option::is_none")]
	#[arg(long = "auth-key")]
	pub key: Option<String>,

	/// A map of paths to key files.
	///
	/// The .jwt token can be prepended with an optional path to use that key instead of the root key.
	#[serde(skip_serializing_if = "Option::is_none")]
	#[arg(long = "auth-path", value_parser = parse_key_val)]
	pub path: Option<HashMap<String, String>>,
}

impl AuthConfig {
	pub fn init(self) -> anyhow::Result<Auth> {
		Auth::new(self)
	}
}

// Only support one key=value pair for now. If you want more, use a config file.
fn parse_key_val(s: &str) -> Result<HashMap<String, String>, String> {
	if s.is_empty() {
		return Ok(HashMap::new());
	}
	let (k, v) = s
		.split_once('=')
		.ok_or_else(|| format!("invalid KEY=VALUE: no `=` in `{}`", s))?;
	let mut map = HashMap::new();
	map.insert(k.to_string(), v.to_string());
	Ok(map)
}

pub struct Auth {
	key: Option<moq_token::Key>,
	paths: Arc<HashMap<String, Option<moq_token::Key>>>,
}

impl Auth {
	pub fn new(config: AuthConfig) -> anyhow::Result<Self> {
		let mut paths = HashMap::new();

		let key = match config.key.as_deref() {
			None | Some("") => {
				tracing::warn!("connection authentication is disabled; users can publish/subscribe to any path");
				None
			}
			Some(path) => {
				let key = moq_token::Key::from_file(path)?;
				anyhow::ensure!(
					key.operations.contains(&moq_token::KeyOperation::Verify),
					"key does not support verification"
				);
				Some(key)
			}
		};

		for (path, file) in config.path.unwrap_or_default() {
			let key = match file.as_ref() {
				"" => None,
				path => {
					let key = moq_token::Key::from_file(path)?;
					anyhow::ensure!(
						key.operations.contains(&moq_token::KeyOperation::Verify),
						"key does not support verification"
					);
					Some(key)
				}
			};

			paths.insert(path, key);
		}

		Ok(Self {
			key,
			paths: Arc::new(paths),
		})
	}

	// Parse/validate a user provided URL.
	pub fn validate(&self, url: &Url) -> anyhow::Result<moq_token::Payload> {
		tracing::trace!(path = url.path(), "validating URL");

		let path = url.path().trim_start_matches('/');
		let (prefix, suffix) = path.rsplit_once("/").unwrap_or(("", path));

		let auth = self.paths.get(prefix).unwrap_or(&self.key);

		if let Some(token) = suffix.strip_suffix(".jwt") {
			let auth = auth.as_ref().expect("no authentication configured");

			// Verify the token and return the payload.
			let mut token = auth.verify(token)?;

			// Add the key ID back to the path.
			token.path = format!("{}{}", prefix, token.path);
			return Ok(token);
		}

		if auth.is_some() {
			return Err(anyhow::anyhow!("no token provided"));
		}

		// No auth required, so create a dummy token that allows accessing everything.
		Ok(moq_token::Payload {
			path: path.to_string(),
			publish: Some("".to_string()),
			subscribe: Some("".to_string()),
			..Default::default()
		})
	}
}
