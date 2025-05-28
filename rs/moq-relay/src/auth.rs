use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use url::Url;

#[serde_with::serde_as]
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct AuthConfig {
	/// A map of paths to key files.
	///
	/// When a user connects, the first segment of the path is used to determine the key to use.
	/// If the value is None, then the user is allowed to do whatever they want.
	pub key: HashMap<String, PathBuf>,
}

impl AuthConfig {
	pub fn init(self) -> anyhow::Result<Auth> {
		Auth::new(self)
	}
}

pub struct Auth {
	tokens: Arc<HashMap<String, Option<moq_token::Key>>>,
}

impl Auth {
	pub fn new(config: AuthConfig) -> anyhow::Result<Self> {
		let mut tokens = HashMap::new();

		for (path, file) in config.key {
			if file == PathBuf::from("") {
				tokens.insert(path, None);
				continue;
			}

			let key = moq_token::Key::from_file(file)?;
			anyhow::ensure!(
				key.operations.contains(&moq_token::KeyOperation::Verify),
				"key does not support verification"
			);
			tokens.insert(path, Some(key));
		}

		Ok(Self {
			tokens: Arc::new(tokens),
		})
	}

	// Parse/validate a user provided URL.
	pub fn validate(&self, url: &Url) -> anyhow::Result<moq_token::Payload> {
		// Scope everything to the session URL path.
		// ex. if somebody connects with `/foo/bar/` then SUBSCRIBE "baz" will return `/foo/bar/baz`.
		let mut segments = url.path_segments().context("missing path")?;
		let key = segments.next().context("missing key")?;

		let auth = self.tokens.get(key).context("unknown key")?;

		if let Some(auth) = auth {
			let token = segments.next().context("missing token")?;
			anyhow::ensure!(segments.next().is_none(), "unexpected path segment");

			// Verify the token and return the payload.
			let mut token = auth.verify(token)?;

			// Add the key ID to the path.
			// We just do this because it's simpler than using a separate `path` field.
			token.path = format!("{}/{}", key, token.path);

			return Ok(token);
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

	pub fn key(&self, key: &str) -> anyhow::Result<Option<&moq_token::Key>> {
		Ok(self.tokens.get(key).context("unknown key")?.as_ref())
	}
}
