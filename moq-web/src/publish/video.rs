use std::time::Duration;

use moq_async::FuturesExt;

use crate::{Error, Result};

pub struct Video {
	media: web_sys::MediaStreamTrack,
	info: moq_karp::Video,
	reader: web_streams::Reader<web_sys::VideoFrame>,
	encoder: web_codecs::VideoEncoder,
	encoded: web_codecs::VideoEncoded,
}

impl Video {
	// Async because we need to initialize the video encoder.
	// Unfortunate, I know, but it should take milliseconds.
	pub async fn new(media: web_sys::MediaStreamTrack) -> Result<Self> {
		let config = Self::config(&media).await?;
		let (mut encoder, encoded) = config.clone().init()?;

		let name = media.id();

		// Unfortunately... we don't know the description right now.
		// We actually need to pump the encoder at least once.
		let init = web_sys::MediaStreamTrackProcessorInit::new(&media);
		let processor = web_sys::MediaStreamTrackProcessor::new(&init).unwrap();
		let mut reader: web_streams::Reader<web_sys::VideoFrame> = web_streams::Reader::new(&processor.readable())?;

		// Pump the encoder to get the first decoded frame, so we can learn the description.
		let decoder_config;
		loop {
			tokio::select! {
				Some(frame) = reader.read().transpose() => {
					let frame = frame?.into();
					encoder.encode(&frame, Default::default())?;
				},
				Some(config) = encoded.config() => {
					tracing::info!("config: {:?}", config);
					decoder_config = config;

					break;
				},
				else => return Err(Error::InitFailed),
			}
		}

		let info = moq_karp::Video {
			track: moq_karp::Track { name, priority: 2 },
			codec: config.codec.into(),
			description: decoder_config.description,
			resolution: moq_karp::Dimensions {
				width: config.resolution.width,
				height: config.resolution.height,
			},
			bitrate: config.bit_rate.map(|b| b as _),
		};

		Ok(Self {
			media,
			info,
			reader,
			encoder,
			encoded,
		})
	}

	pub fn info(&self) -> &moq_karp::Video {
		&self.info
	}

	pub async fn frame(&mut self) -> Result<Option<moq_karp::Frame>> {
		loop {
			tokio::select! {
				// A new input frame is available.
				Some(frame) = self.reader.read().transpose() => {
					let frame: web_codecs::VideoFrame = frame?.into(); // NOTE: closed on Drop
					self.encoder.encode(&frame, Default::default())?;
				},
				// A new output frame is available.
				Some(frame) = self.encoded.frame().transpose() => {
					let frame = frame?;
					// TODO combine these types one day?
					let frame = moq_karp::Frame {
						timestamp: frame.timestamp,
						keyframe: frame.keyframe,
						payload: frame.payload,
					};
					return Ok(Some(frame));
				},
				// All done with both the input and output.
				else => return Ok(None),
			}
		}
	}

	async fn config(track: &web_sys::MediaStreamTrack) -> Result<web_codecs::VideoEncoderConfig> {
		let settings = track.get_settings();
		tracing::info!(?settings);

		let resolution = web_codecs::Dimensions {
			width: settings.get_width().ok_or(web_codecs::Error::InvalidDimensions)? as _,
			height: settings.get_height().ok_or(web_codecs::Error::InvalidDimensions)? as _,
		};

		tracing::info!(?resolution);

		let base = web_codecs::VideoEncoderConfig {
			resolution,                  // TODO configurable
			bit_rate: Some(2_000_000.0), // TODO configurable
			display: Some(resolution),
			latency_optimized: Some(true), // TODO configurable
			frame_rate: settings.get_frame_rate(),
			max_gop_duration: Some(Duration::from_secs(2)),
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

				if config.is_supported().await? {
					return Ok(config);
				}
			}
		}

		Err(Error::Unsupported)
	}
}

impl Drop for Video {
	fn drop(&mut self) {
		// Terminate when one of the handles is dropped.
		self.media.stop();
	}
}
