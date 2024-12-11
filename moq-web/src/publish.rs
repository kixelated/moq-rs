use moq_karp::moq_transfork;
use url::Url;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::{Error, Result};

// A list of codecs and profiles sorted in preferred order.
// TODO automate this list by looping over profile/level pairs
const VIDEO_CODECS: &[&'static str] = &[
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

#[wasm_bindgen]
pub struct Publish {
	broadcast: moq_karp::BroadcastProducer,
	video_id: usize,
}

#[wasm_bindgen]
impl Publish {
	pub async fn connect(src: &str) -> Result<Self> {
		let src = Url::parse(src).map_err(|_| Error::InvalidUrl)?;
		let session = crate::session::connect(&src).await?;
		let path: moq_transfork::Path = src.path_segments().ok_or(Error::InvalidUrl)?.collect();
		let broadcast = moq_karp::BroadcastProducer::new(session, path)?;

		Ok(Self { broadcast, video_id: 0 })
	}

	pub async fn video(&mut self, settings: web_sys::MediaTrackSettings) -> Result<PublishVideo> {
		let config = Self::video_config(settings).await?;

		// Unfortunately, we can't publish yet because we need to initilize the encoder.
		let track = PublishVideo::new(self.broadcast.clone(), config, self.video_id)?;
		self.video_id += 1;

		Ok(track)
	}

	async fn video_config(settings: web_sys::MediaTrackSettings) -> Result<web_codecs::VideoEncoderConfig> {
		let resolution = web_codecs::Dimensions {
			width: settings.get_width().ok_or(web_codecs::Error::InvalidDimensions)? as _,
			height: settings.get_height().ok_or(web_codecs::Error::InvalidDimensions)? as _,
		};

		let base = web_codecs::VideoEncoderConfig {
			resolution,                  // TODO configurable
			bit_rate: Some(2_000_000.0), // TODO configurable
			display: Some(resolution),
			latency_optimized: Some(true), // TODO configurable
			frame_rate: settings.get_frame_rate(),
			..Default::default()
		};

		// First check if the browser supports hardware acceleration, trying the best configurations first.
		for accelerated in [true, false] {
			for codec in VIDEO_CODECS {
				let config = web_codecs::VideoEncoderConfig {
					codec: codec.to_string(),
					hardware_acceleration: Some(accelerated),
					..base.clone()
				};

				if config.is_supported().await? {
					return Ok(config);
				}
			}
		}

		Err(Error::Unsupported)
	}

	pub fn close(&mut self) {
		// TODO
	}
}

#[wasm_bindgen]
pub struct PublishVideo {
	encoder: Option<web_codecs::VideoEncoder>,
}

#[wasm_bindgen]
impl PublishVideo {
	fn new(broadcast: moq_karp::BroadcastProducer, config: web_codecs::VideoEncoderConfig, id: usize) -> Result<Self> {
		let (encoder, encoded) = config.clone().init()?;

		spawn_local(async move {
			if let Err(err) = Self::run(broadcast, config, encoded, id).await {
				tracing::error!(?err, "publishing failed");
			}
		});

		Ok(Self { encoder: Some(encoder) })
	}

	async fn run(
		mut broadcast: moq_karp::BroadcastProducer,
		encoder: web_codecs::VideoEncoderConfig,
		mut encoded: web_codecs::VideoEncoded,
		id: usize,
	) -> Result<()> {
		let config = encoded.config().await.ok_or(Error::InitFailed)?;

		let video = moq_karp::Video {
			track: moq_karp::Track {
				name: format!("video{}", id),
				priority: 2,
			},
			codec: config.codec.try_into().unwrap(),
			description: config.description,
			resolution: moq_karp::Dimensions {
				width: encoder.resolution.width,
				height: encoder.resolution.height,
			},
			bitrate: encoder.bit_rate,
		};

		let mut track = broadcast.publish_video(video)?;

		while let Some(frame) = encoded.frame().await? {
			track.write(moq_karp::Frame {
				timestamp: moq_karp::Timestamp::from_micros(frame.timestamp as _),
				keyframe: frame.keyframe,
				payload: frame.payload,
			});
		}

		Ok(())
	}

	pub async fn encode(&mut self, frame: web_sys::VideoFrame) -> Result<()> {
		self.encoder.as_mut().ok_or(Error::Closed)?.encode(frame.into())?;
		Ok(())
	}

	pub fn close(&mut self) {
		self.encoder.take();
	}
}
