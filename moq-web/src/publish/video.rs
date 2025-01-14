use moq_async::FuturesExt;
use wasm_bindgen_futures::spawn_local;

use crate::{Error, Result};

#[derive(Clone)]
pub struct Video {
	broadcast: moq_karp::BroadcastProducer,
	track: web_sys::MediaStreamTrack,
}

impl Video {
	pub fn new(broadcast: moq_karp::BroadcastProducer, track: web_sys::MediaStreamTrack) -> Self {
		let this = Self { broadcast, track };
		let task = this.clone();

		spawn_local(async move {
			if let Err(err) = task.run().await {
				tracing::error!(?err, "publishing failed");
			}
		});

		this
	}

	async fn run(mut self) -> Result<()> {
		let config = self.config().await?;
		let (mut encoder, mut encoded) = config.clone().init()?;

		let name = self.track.id();

		// Unfortunately... we don't know the description right now.
		// We actually need to pump the encoder at least once.
		let init = web_sys::MediaStreamTrackProcessorInit::new(&self.track);
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
				else => return Err(Error::CaptureFailed),
			}
		}

		let track = moq_karp::Video {
			track: moq_karp::Track { name, priority: 2 },
			codec: config.codec.try_into().unwrap(),
			description: decoder_config.description,
			resolution: moq_karp::Dimensions {
				width: config.resolution.width,
				height: config.resolution.height,
			},
			bitrate: config.bit_rate,
		};

		tracing::info!(?track);

		let mut track = self.broadcast.publish_video(track)?;

		loop {
			tokio::select! {
				// A new input frame is available.
				Some(frame) = reader.read().transpose() => {
					let frame: web_codecs::VideoFrame = frame?.into(); // NOTE: closed on Drop
					encoder.encode(&frame, Default::default())?;
				},
				// A new output frame is available.
				Some(frame) = encoded.frame().transpose() => {
					let frame = frame?;
					track.write(moq_karp::Frame {
						timestamp: moq_karp::Timestamp::from_micros(frame.timestamp.as_micros()),
						keyframe: frame.keyframe,
						payload: frame.payload,
					});
				},
				// All done with both the input and output.
				else => break,
			}
		}

		Ok(())
	}

	async fn config(&self) -> Result<web_codecs::VideoEncoderConfig> {
		let settings = self.track.get_settings();
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
			max_gop_duration: Some(web_codecs::Duration::from_seconds(2)),
			..Default::default()
		};

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
		self.track.stop();
	}
}
