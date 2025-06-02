use std::{collections::HashMap, sync::Arc};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use url::Url;

#[serde_with::serde_as]
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AuthConfig {
	/// The root key to use for all connections.
	///
	/// This is the fallback if a path does not exist in the `path` map below.
	/// If this is missing, then authentication is completely disabled, even if a path is configured below.
	#[serde(skip_serializing_if = "String::is_empty")]
	pub root: String,

	/// A map of paths to key files.
	///
	/// The .jwt token can be prepended with an optional path to use that key instead of the root key.
	#[serde(skip_serializing_if = "HashMap::is_empty")]
	pub path: HashMap<String, String>,
}

impl AuthConfig {
	pub fn init(self) -> anyhow::Result<Auth> {
		Auth::new(self)
	}
}

pub struct Auth {
	root: Option<moq_token::Key>,
	paths: Arc<HashMap<String, Option<moq_token::Key>>>,
}

impl Auth {
	pub fn new(config: AuthConfig) -> anyhow::Result<Self> {
		let mut paths = HashMap::new();

		let root = match config.root.as_ref() {
			"" => {
				anyhow::ensure!(config.path.is_empty(), "root key is empty but paths are configured");
				tracing::warn!("authentication is disabled");
				None
			}
			path => {
				let key = moq_token::Key::from_file(path)?;
				anyhow::ensure!(
					key.operations.contains(&moq_token::KeyOperation::Verify),
					"key does not support verification"
				);
				Some(key)
			}
		};

		for (path, file) in config.path {
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
			root,
			paths: Arc::new(paths),
		})
	}

	// Parse/validate a user provided URL.
	pub fn validate(&self, url: &Url) -> anyhow::Result<moq_token::Payload> {
		let segments = url.path_segments().context("missing path")?.collect::<Vec<_>>();

		if let Some(token) = segments.last().unwrap().strip_suffix(".jwt") {
			let path = segments[..segments.len() - 1].join("/");

			// As a precaution, reject all incoming connections that expected authentication.
			anyhow::ensure!(self.root.is_some(), "root key is required for authenticated URLs");

			if let Some(auth) = self.paths.get(&path).unwrap_or(&self.root) {
				// Verify the token and return the payload.
				let mut token = auth.verify(token)?;

				// Add the key ID back to the path.
				token.path = format!("{}/{}", path, token.path);
				return Ok(token);
			}
		}

		// No auth required, so create a dummy token that allows accessing everything.
		Ok(moq_token::Payload {
			// Use the user-provided path.
			path: url.path().trim_start_matches('/').to_string(),
			publish: Some("".to_string()),
			subscribe: Some("".to_string()),
			..Default::default()
		})
	}
}
