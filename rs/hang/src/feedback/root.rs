use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::Capabilities;
use crate::Result;

/// A feedback track, created by a viewer to inform broadcasters.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Feedback {
	/// A capabilities track, indicating our viewing capabilities.
	/// Another broadcaster MAY use this information to change codecs, bitrate, etc.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub capabilities: Option<Capabilities>,

	/// If the broadcaster specified a location handle, we can suggest new locations.
	#[serde(default, skip_serializing_if = "HashMap::is_empty")]
	pub locations: HashMap<u32, moq_lite::Track>,
}

impl Feedback {
	pub const DEFAULT_NAME: &str = "feedback.json";

	#[allow(clippy::should_implement_trait)]
	pub fn from_str(s: &str) -> Result<Self> {
		Ok(serde_json::from_str(s)?)
	}

	pub fn from_slice(v: &[u8]) -> Result<Self> {
		Ok(serde_json::from_slice(v)?)
	}

	pub fn from_reader(reader: impl std::io::Read) -> Result<Self> {
		Ok(serde_json::from_reader(reader)?)
	}

	pub fn to_string(&self) -> Result<String> {
		Ok(serde_json::to_string(self)?)
	}

	pub fn to_string_pretty(&self) -> Result<String> {
		Ok(serde_json::to_string_pretty(self)?)
	}

	pub fn to_vec(&self) -> Result<Vec<u8>> {
		Ok(serde_json::to_vec(self)?)
	}

	pub fn to_writer(&self, writer: impl std::io::Write) -> Result<()> {
		Ok(serde_json::to_writer(writer, self)?)
	}
}
