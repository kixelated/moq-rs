use hang::{moq_lite, BroadcastProducer, Room};
use std::time::Duration;
use ts_rs::TS;
use web_message::Message;

use crate::{Error, Result};

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/rpc.ts")]
pub enum PublishCommand {
	Join {
		name: String,
	},

	#[ts(type = "AudioData")]
	AudioFrame(web_sys::AudioData),
	AudioClose,

	#[ts(type = "VideoFrame")]
	VideoFrame(web_sys::VideoFrame),
	VideoClose,
}

pub struct Publish {
	room: Option<Room>,
	name: Option<String>,

	broadcast: Option<BroadcastProducer>,
	video: Option<PublishVideo>,
	video_id: usize,
}

impl Default for Publish {
	fn default() -> Self {
		Self::new()
	}
}

impl Publish {
	pub fn new() -> Self {
		Self {
			room: None,
			broadcast: None,
			name: None,
			video: None,
			video_id: 0,
		}
	}

	pub async fn recv_command(&mut self, command: PublishCommand) -> Result<()> {
		match command {
			PublishCommand::Join { name } => {
				self.name = Some(name);
				self.create_broadcast()?;
			}
			//PublishCommand::Audio(audio) => self.audio = Some(audio),
			//PublishCommand::AudioReset => self.audio = None,
			PublishCommand::VideoFrame(frame) => {
				// Don't encode anything until connecting to a room.
				// This is done for privacy reasons, just in case.
				match self.video.as_mut() {
					Some(video) => video.encode(frame).await?,
					None => frame.close(),
				};
			}
			PublishCommand::VideoClose => {
				self.video = None;
			}
			PublishCommand::AudioFrame(_audio) => {
				//self.audio = Some(audio);
			}
			PublishCommand::AudioClose => {
				//self.audio = None;
			}
		};

		Ok(())
	}

	pub fn set_room(&mut self, room: Option<Room>) -> Result<()> {
		self.room = room;
		self.create_broadcast()
	}

	fn create_broadcast(&mut self) -> Result<()> {
		if let (Some(room), Some(name)) = (self.room.as_mut(), self.name.as_ref()) {
			let broadcast = room.join(name.clone())?;

			self.video = Some(PublishVideo::new(broadcast.clone(), self.video_id));
			self.video_id += 1;

			self.broadcast = Some(broadcast);
		} else {
			self.broadcast = None;
		}

		Ok(())
	}

	pub async fn run(&mut self) -> Result<()> {
		loop {
			tokio::select! {
				Some(Err(err)) = async { Some(self.video.as_mut()?.run().await) } => return Err(err),
			}
		}
	}
}

pub struct PublishVideo {
	// The broadcast that we are publishing to.
	broadcast: hang::BroadcastProducer,

	// The config that we are using for the encoder, initialized on the first frame.
	config: Option<web_codecs::VideoEncoderConfig>,

	// The track that we are publishing.
	// We start writing frames before the encoder has been fully configured.
	track: hang::TrackProducer,

	// The encoder accepts raw frames and spits out encoded frames.
	encoder: Option<web_codecs::VideoEncoder>,
	encoded: Option<web_codecs::VideoEncoded>,

	// Set to true after we successfully publish the track.
	published: bool,
}

impl PublishVideo {
	pub fn new(broadcast: hang::BroadcastProducer, id: usize) -> Self {
		let track = moq_lite::Track {
			name: format!("video_{}", id),
			priority: 2,
		};

		Self {
			broadcast,
			config: None,
			track: hang::TrackProducer::new(track.into()),
			encoder: None,
			encoded: None,
			published: false,
		}
	}

	pub async fn encode(&mut self, frame: web_sys::VideoFrame) -> Result<()> {
		let frame = frame.into();

		match self.encoder.as_mut() {
			Some(encoder) => {
				encoder.encode(&frame, Default::default())?;
			}
			None => {
				let config = Self::config(&frame).await?;
				let (mut encoder, encoded) = config.clone().init()?;
				encoder.encode(&frame, web_codecs::VideoEncodeOptions { key_frame: Some(true) })?;

				self.config = Some(config);
				self.encoder = Some(encoder);
				self.encoded = Some(encoded);
			}
		};

		Ok(())
	}

	/// Block until the decoder has enough information about the encoding.
	async fn info(&self) -> Option<hang::Video> {
		let config = self.encoded.as_ref()?.config().await?;

		let resolution = config.resolution.map(|r| hang::Dimensions {
			width: r.width,
			height: r.height,
		});

		Some(hang::Video {
			track: self.track.inner.info.clone(),
			codec: config.codec.into(),
			description: config.description,
			resolution,
			bitrate: self.config.as_ref()?.bit_rate.map(|b| b as _),
		})
	}

	pub async fn run(&mut self) -> Result<()> {
		if !self.published {
			let info = match self.info().await {
				Some(info) => info,
				// Not configured yet.
				None => return Ok(()),
			};

			self.broadcast.add_video(self.track.consume(), info);
			self.published = true;
		}

		while let Some(frame) = self.encoded.as_mut().unwrap().frame().await? {
			self.track.write(hang::Frame {
				timestamp: frame.timestamp,
				keyframe: frame.keyframe,
				payload: frame.payload,
			});
		}

		Ok(())
	}

	async fn config(frame: &web_codecs::VideoFrame) -> Result<web_codecs::VideoEncoderConfig> {
		let resolution = web_codecs::Dimensions {
			width: frame.coded_width(),
			height: frame.coded_height(),
		};

		let base = web_codecs::VideoEncoderConfig {
			resolution,                  // TODO configurable
			bit_rate: Some(2_000_000.0), // TODO configurable
			display: Some(resolution),
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
}
