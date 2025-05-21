use hang::moq_lite;
use std::time::Duration;

use crate::{Error, Result};
pub struct PublishVideo {
	// The config that we are using for the encoder, initialized on the first frame.
	config: web_codecs::VideoEncoderConfig,

	// The track that we are publishing.
	track: hang::TrackProducer,

	// The encoder accepts raw frames and spits out encoded frames.
	encoder: web_codecs::VideoEncoder,
	encoded: web_codecs::VideoEncoded,

	// When set, publish to the given broadcast.
	broadcast: Option<hang::BroadcastProducer>,
}

impl PublishVideo {
	pub async fn init(id: usize, width: u32, height: u32) -> Result<Self> {
		let track = moq_lite::Track {
			name: format!("video_{}", id),
			priority: 2,
		}
		.produce();

		let config = Self::config(width, height).await?;
		let (encoder, encoded) = config.clone().init()?;

		Ok(Self {
			config,
			track: track.into(),
			encoder,
			encoded,
			broadcast: None,
		})
	}

	async fn config(width: u32, height: u32) -> Result<web_codecs::VideoEncoderConfig> {
		let base = web_codecs::VideoEncoderConfig {
			resolution: web_codecs::Dimensions { width, height },
			bitrate: Some(2_000_000),      // TODO configurable
			latency_optimized: Some(true), // TODO configurable
			max_gop_duration: Some(Duration::from_secs(4)),
			..Default::default()
		};

		// A list of codecs and profiles sorted in preferred order.
		// TODO automate this list by looping over profile/level pairs
		const VIDEO_CODECS: &[&str] = &[
			// AV1 Main Profile, level 3.0, Main tier, 8 bits
			"av01.0.04M.08",
			// HEVC Main10 Profile, Main Tier, Level 4.0
			"hev1.2.4.L120.B0",
			// AVC High Level 3
			"avc1.64001e",
			// AVC High Level 4
			"avc1.640028",
			// AVC High Level 5
			"avc1.640032",
			// AVC High Level 5.2
			"avc1.640034",
			// AVC Main Level 3
			"avc1.4d001e",
			// AVC Main Level 4
			"avc1.4d0028",
			// AVC Main Level 5
			"avc1.4d0032",
			// AVC Main Level 5.2
			"avc1.4d0034",
			// AVC Baseline Level 3
			"avc1.42001e",
			// AVC Baseline Level 4
			"avc1.420028",
			// AVC Baseline Level 5
			"avc1.420032",
			// AVC Baseline Level 5.2
			"avc1.420034",
		];

		// First check if the browser supports hardware acceleration, trying the best configurations first.
		for accelerated in [true, false] {
			for codec in VIDEO_CODECS {
				let config = web_codecs::VideoEncoderConfig {
					codec: codec.to_string(),
					hardware_acceleration: Some(accelerated),
					..base.clone()
				};

				// TODO This is async, but for now we block on it.
				// TODO block on it
				if config.is_supported().await? {
					return Ok(config);
				}
			}
		}

		Err(Error::Unsupported)
	}

	pub async fn encode(&mut self, frame: web_sys::VideoFrame) -> Result<()> {
		let frame = frame.into();
		self.encoder.encode(&frame, Default::default())?;

		Ok(())
	}

	pub async fn run(&mut self) -> Result<()> {
		while let Some(frame) = self.encoded.frame().await? {
			if let Some(mut broadcast) = self.broadcast.take() {
				self.publish_to(&mut broadcast);
			}

			self.track.write(hang::Frame {
				timestamp: frame.timestamp,
				keyframe: frame.keyframe,
				payload: frame.payload,
			});
		}

		Ok(())
	}

	pub fn publish_to(&mut self, broadcast: &mut hang::BroadcastProducer) {
		if let Some(config) = self.encoded.config() {
			let dimensions = config.resolution.map(|r| hang::Dimensions {
				width: r.width,
				height: r.height,
			});

			let info = hang::Video {
				track: self.track.inner.info.clone(),
				codec: config.codec.into(),
				description: config.description,
				dimensions,
				bitrate: self.config.bitrate.map(|b| b as _),
				framerate: self.config.framerate.map(|f| f as _),
				display_ratio: None,
				rotation: None,
				flip: None,
				optimize_for_latency: None,
			};

			broadcast.add_video(self.track.consume(), info);
		} else {
			// Save a reference for later after fully initializing.
			self.broadcast = Some(broadcast.clone());
		}
	}
}
