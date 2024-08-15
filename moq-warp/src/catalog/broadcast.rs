use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{Audio, Container, Error, Result, Video};

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Broadcast {
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub video: Vec<Video>,

	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub audio: Vec<Audio>,

	#[serde(skip_serializing_if = "HashMap::is_empty")]
	#[serde_with(as = "HashMap<_, serde_with::hex::Hex>")]
	pub init: HashMap<Container, Vec<u8>>,
}

impl Broadcast {
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

	pub async fn fetch(broadcast: moq_transfork::BroadcastConsumer) -> Result<Self> {
		let track = moq_transfork::Track::build("catalog.json")
			.priority(-1)
			.group_order(moq_transfork::GroupOrder::Desc)
			.group_expires(std::time::Duration::ZERO)
			.into();

		let mut track = broadcast.subscribe(track).await?;
		let mut group = track.next_group().await?.ok_or(Error::Empty)?;
		let frame = group.read_frame().await?.ok_or(Error::Empty)?;
		let parsed = Self::from_slice(&frame)?;

		Ok(parsed)
	}

	pub fn publish(&self, broadcast: &mut moq_transfork::BroadcastProducer) -> Result<()> {
		let track = moq_transfork::Track::build("catalog.json")
			.priority(-1)
			.group_order(moq_transfork::GroupOrder::Desc)
			.group_expires(std::time::Duration::ZERO)
			.into();

		let mut track = broadcast.insert_track(track);
		let mut group = track.append_group();

		let frame = self.to_string()?;
		group.write_frame(frame.into());

		Ok(())
	}
}

#[cfg(test)]
mod test {
	use crate::catalog::{self, AudioCodec};

	use super::*;

	#[test]
	fn simple() {
		let encoded = r#"{
			"video": [
				{
					"track": {
						"name": "video",
						"priority": 1,
						"group_order": "desc",
						"group_expires": 0
					},
					"container": "fmp4",
					"codec": "avc1.64001f",
					"bitrate": 6000000,
					"dimensions": {
						"width": 1280,
						"height": 720
					},
					"display": {
						"width": 1920,
						"height": 1080
					},
				}
			],
			"audio": [
				{
					"track": {
						"name": "audio",
						"priority": 0,
						"group_order": "desc",
						"group_expires": 0
					},
					"container": "fmp4",
					"codec": "opus",
					"sample_rate": 48000,
					"channels": 2,
					"bitrate": 128000,
				}
			],
			"init": {
				"fmp4": "000000147374797069736f6d0000020069736f6d000000086d6f6f76",
			}
		}"#;

		let decoded = Broadcast {
			video: vec![Video {
				track: "video".into(),
				container: Container::Fmp4,
				codec: catalog::H264 {
					profile: 0x64,
					constraints: 0x00,
					level: 0x1f,
				}
				.into(),
				dimensions: catalog::Dimensions {
					width: 1280,
					height: 720,
				},
				display: Some(catalog::Dimensions {
					width: 1920,
					height: 1080,
				}),
				layers: Default::default(),
				bit_rate: Some(6_000_000),
			}],
			audio: vec![Audio {
				track: "audio".into(),
				container: Container::Fmp4,
				codec: AudioCodec::Opus,
				sample_rate: 48000,
				channel_count: 2,
				bit_rate: Some(128_000),
			}],
			init: HashMap::from([(
				Container::Fmp4,
				vec![
					0x00, 0x00, 0x00, 0x14, 0x73, 0x74, 0x79, 0x70, 0x69, 0x73, 0x6F, 0x6D, 0x00, 0x00, 0x02, 0x00,
					0x69, 0x73, 0x6F, 0x6D, 0x00, 0x00, 0x00, 0x08, 0x6D, 0x6F, 0x6F, 0x76,
				],
			)]),
		};

		let output = Broadcast::from_str(&encoded).expect("failed to decode");
		assert_eq!(decoded, output, "wrong decoded output");

		let output = decoded.to_string_pretty().expect("failed to encode");
		assert_eq!(encoded, output, "wrong encoded output");
	}
}
