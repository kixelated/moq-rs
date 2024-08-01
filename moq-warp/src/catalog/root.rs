use serde::{Deserialize, Serialize};

use super::{CommonTrackFields, Result, Track};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Root {
	pub version: u16,

	#[serde(rename = "streamingFormat")]
	pub streaming_format: u16,

	#[serde(rename = "streamingFormatVersion")]
	pub streaming_format_version: String,

	#[serde(rename = "supportsDeltaUpdates")]
	pub streaming_delta_updates: bool,

	#[serde(rename = "commonTrackFields")]
	pub common_track_fields: CommonTrackFields,

	pub tracks: Vec<Track>,
}

impl Root {
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
