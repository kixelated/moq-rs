//! This module contains the structs and functions for the MoQ catalog format
use std::sync::{Arc, Mutex};

/// The catalog format is a JSON file that describes the tracks available in a broadcast.
use serde::{Deserialize, Serialize};

use crate::catalog::{AudioTrack, VideoTrack};
use crate::Result;

use super::{Capabilities, LocationTrack};

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Root {
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub video: Vec<VideoTrack>,

	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub audio: Vec<AudioTrack>,

	/// A location track, used to indicate the desired position of the broadcaster from -1 to 1.
	/// This is primarily used for audio panning but can also be used for video.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub location: Option<LocationTrack>,

	/// A capabilities track, indicating our viewing capabilities.
	/// Another broadcaster MAY use this information to change codecs, bitrate, etc.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub capabilities: Option<Capabilities>,
}

impl Root {
	pub const DEFAULT_NAME: &str = "catalog.json";

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

	pub fn produce(self) -> CatalogProducer {
		let track = moq_lite::Track {
			name: Root::DEFAULT_NAME.to_string(),
			priority: 100,
		}
		.produce();

		CatalogProducer::new(track, self)
	}
}

#[derive(Clone)]
pub struct CatalogProducer {
	pub track: moq_lite::TrackProducer,
	current: Arc<Mutex<Root>>,
}

impl CatalogProducer {
	pub fn new(track: moq_lite::TrackProducer, init: Root) -> Self {
		Self {
			current: Arc::new(Mutex::new(init)),
			track,
		}
	}

	pub fn add_video(&mut self, video: VideoTrack) {
		let mut current = self.current.lock().unwrap();
		current.video.push(video);
	}

	pub fn add_audio(&mut self, audio: AudioTrack) {
		let mut current = self.current.lock().unwrap();
		current.audio.push(audio);
	}

	pub fn set_location(&mut self, location: Option<LocationTrack>) {
		let mut current = self.current.lock().unwrap();
		current.location = location;
	}

	pub fn set_capabilities(&mut self, capabilities: Option<Capabilities>) {
		let mut current = self.current.lock().unwrap();
		current.capabilities = capabilities;
	}

	pub fn remove_video(&mut self, video: &VideoTrack) {
		let mut current = self.current.lock().unwrap();
		current.video.retain(|v| v != video);
	}

	pub fn remove_audio(&mut self, audio: &AudioTrack) {
		let mut current = self.current.lock().unwrap();
		current.audio.retain(|a| a != audio);
	}

	/// Publish any changes to the catalog.
	pub fn publish(&mut self) {
		let current = self.current.lock().unwrap();
		let mut group = self.track.append_group();

		// TODO decide if this should return an error, or be impossible to fail
		let frame = current.to_string().expect("invalid catalog");
		group.write_frame(frame);
		group.finish();
	}

	pub fn consume(&self) -> CatalogConsumer {
		CatalogConsumer::new(self.track.consume())
	}
}

impl From<moq_lite::TrackProducer> for CatalogProducer {
	fn from(inner: moq_lite::TrackProducer) -> Self {
		Self::new(inner, Root::default())
	}
}

impl Default for CatalogProducer {
	fn default() -> Self {
		let track = moq_lite::Track {
			name: Root::DEFAULT_NAME.to_string(),
			priority: 100,
		}
		.produce();

		CatalogProducer::new(track, Root::default())
	}
}

#[derive(Clone)]
pub struct CatalogConsumer {
	pub track: moq_lite::TrackConsumer,
	group: Option<moq_lite::GroupConsumer>,
}

impl CatalogConsumer {
	pub fn new(track: moq_lite::TrackConsumer) -> Self {
		Self { track, group: None }
	}

	pub async fn next(&mut self) -> Result<Option<Root>> {
		loop {
			tokio::select! {
				res = self.track.next_group() => {
					match res? {
						Some(group) => {
							// Use the new group.
							self.group = Some(group);
						}
						None => {
							// The track has ended, so we should return None.
							return Ok(None);
						}
					}
				},
				Some(frame) = async { self.group.as_mut()?.read_frame().await.transpose() } => {
					self.group.take(); // We don't support deltas yet
					let catalog = Root::from_slice(&frame?)?;
					return Ok(Some(catalog));
				}
			}
		}
	}
}

impl From<moq_lite::TrackConsumer> for CatalogConsumer {
	fn from(inner: moq_lite::TrackConsumer) -> Self {
		Self::new(inner)
	}
}

#[cfg(test)]
mod test {
	use crate::catalog::{AudioCodec::Opus, AudioConfig, VideoConfig, H264};
	use moq_lite::Track;

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
					"config": {
						"codec": "avc1.64001f",
						"codedWidth": 1280,
						"codedHeight": 720,
						"bitrate": 6000000,
						"framerate": 30.0
					}
				}
			],
			"audio": [
				{
					"track": {
						"name": "audio",
						"priority": 1
					},
					"config": {
						"codec": "opus",
						"sampleRate": 48000,
						"numberOfChannels": 2,
						"bitrate": 128000
					}
				}
			]
		}"#
		.to_string();

		encoded.retain(|c| !c.is_whitespace());

		let decoded = Root {
			video: vec![VideoTrack {
				track: Track {
					name: "video".to_string(),
					priority: 1,
				},
				config: VideoConfig {
					codec: H264 {
						profile: 0x64,
						constraints: 0x00,
						level: 0x1f,
					}
					.into(),
					description: None,
					coded_width: Some(1280),
					coded_height: Some(720),
					display_ratio_width: None,
					display_ratio_height: None,
					bitrate: Some(6_000_000),
					framerate: Some(30.0),
					optimize_for_latency: None,
					rotation: None,
					flip: None,
				},
			}],
			audio: vec![AudioTrack {
				track: Track {
					name: "audio".to_string(),
					priority: 2,
				},
				config: AudioConfig {
					codec: Opus,
					sample_rate: 48_000,
					channel_count: 2,
					bitrate: Some(128_000),
					description: None,
				},
			}],
			..Default::default()
		};

		let output = Root::from_str(&encoded).expect("failed to decode");
		assert_eq!(decoded, output, "wrong decoded output");

		let output = decoded.to_string().expect("failed to encode");
		assert_eq!(encoded, output, "wrong encoded output");
	}
}
