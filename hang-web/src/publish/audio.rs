use hang::moq_lite;

use crate::{Error, Result};

pub struct PublishAudio {
	// The config that we are using for the encoder.
	config: web_codecs::AudioEncoderConfig,

	// The track that we are publishing.
	track: hang::TrackProducer,

	// The encoder accepts raw frames and spits out encoded frames.
	encoder: web_codecs::AudioEncoder,
	encoded: web_codecs::AudioEncoded,

	// When set, publish to the given broadcast.
	broadcast: Option<hang::BroadcastProducer>,
}

impl PublishAudio {
	pub async fn init(id: usize, channel_count: u32, sample_rate: u32) -> Result<Self> {
		let track = moq_lite::Track {
			name: format!("audio_{}", id),
			priority: 1,
		}
		.produce();

		let config = Self::config(channel_count, sample_rate).await?;
		let (encoder, encoded) = config.clone().init()?;

		Ok(Self {
			config,
			track: track.into(),
			encoder,
			encoded,
			broadcast: None,
		})
	}

	pub async fn encode(&mut self, frame: web_sys::AudioData) -> Result<()> {
		let frame = frame.into();
		self.encoder.encode(&frame)?;

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

	async fn config(channel_count: u32, sample_rate: u32) -> Result<web_codecs::AudioEncoderConfig> {
		let config = web_codecs::AudioEncoderConfig {
			codec: "opus".to_string(), // TODO more codecs
			sample_rate: Some(sample_rate as _),
			channel_count: Some(channel_count),
			bitrate: Some(128_000), // TODO configurable
		};

		if config.is_supported().await? {
			Ok(config)
		} else {
			Err(Error::Unsupported)
		}
	}

	pub fn publish_to(&mut self, broadcast: &mut hang::BroadcastProducer) {
		if let Some(config) = self.encoded.config() {
			let info = hang::Audio {
				track: self.track.inner.info.clone(),
				description: config.description,
				codec: config.codec.into(),
				sample_rate: config.sample_rate,
				channel_count: config.channel_count,
				bitrate: self.config.bitrate.map(|b| b as _),
			};

			broadcast.add_audio(self.track.consume(), info);
		} else {
			// Save a reference for later after fully initializing.
			self.broadcast = Some(broadcast.clone());
		}
	}
}
