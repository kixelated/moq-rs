//! This module contains the structs and functions for the MoQ catalog format
/// The catalog format is a JSON file that describes the tracks available in a broadcast.
use serde::{Deserialize, Serialize};

use crate::{Audio, Result, Video};

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Catalog {
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub video: Vec<Video>,

	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub audio: Vec<Audio>,
}

impl Catalog {
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

	pub fn is_empty(&self) -> bool {
		self.video.is_empty() && self.audio.is_empty()
	}
}

#[cfg(test)]
mod test {
	use crate::{AudioCodec::Opus, Dimensions, Track, H264};

	use super::*;

	#[test]
	fn simple() {
		let mut encoded = r#"{
			"video": [
				{
					"track": {
						"name": "video",
						"priority": 2
					},
					"codec": "avc1.64001f",
					"resolution": {
						"width": 1280,
						"height": 720
					},
					"bitrate": 6000000
				}
			],
			"audio": [
				{
					"track": {
						"name": "audio",
						"priority": 1
					},
					"codec": "opus",
					"sample_rate": 48000,
					"channel_count": 2,
					"bitrate": 128000
				}
			]
		}"#
		.to_string();

		encoded.retain(|c| !c.is_whitespace());

		let decoded = Catalog {
			video: vec![Video {
				track: Track {
					name: "video".to_string(),
					priority: 2,
				},
				codec: H264 {
					profile: 0x64,
					constraints: 0x00,
					level: 0x1f,
				}
				.into(),
				description: Default::default(),
				resolution: Dimensions {
					width: 1280,
					height: 720,
				},
				bitrate: Some(6_000_000),
			}],
			audio: vec![Audio {
				track: Track {
					name: "audio".to_string(),
					priority: 1,
				},
				codec: Opus,
				sample_rate: 48_000,
				channel_count: 2,
				bitrate: Some(128_000),
			}],
		};

		let output = Catalog::from_str(&encoded).expect("failed to decode");
		assert_eq!(decoded, output, "wrong decoded output");

		let output = decoded.to_string().expect("failed to encode");
		assert_eq!(encoded, output, "wrong encoded output");
	}
}
