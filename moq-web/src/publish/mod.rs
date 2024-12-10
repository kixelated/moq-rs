use baton::Baton;
use moq_async::FuturesExt;
use moq_karp::moq_transfork;
use url::Url;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::MediaStream;

use crate::{Error, Result};

#[wasm_bindgen]
pub struct Publish {
	controls: ControlsSend,
	status: StatusRecv,
}

#[wasm_bindgen]
impl Publish {
	#[wasm_bindgen(constructor)]
	pub fn new(src: &str) -> Result<Self> {
		let src = Url::parse(src).map_err(|_| Error::InvalidUrl)?;

		let controls = Controls::default().baton();
		let status = Status::default().baton();
		let mut backend = PublishBackend::new(src, controls.1, status.0);

		spawn_local(async move {
			if let Err(err) = backend.run().await {
				tracing::error!(?err, "backend error");
			} else {
				tracing::warn!("backend closed");
			}
		});

		Ok(Self {
			controls: controls.0,
			status: status.1,
		})
	}

	pub fn capture(&mut self, media: Option<MediaStream>) {
		self.controls.media.send(media).ok();
	}

	pub fn volume(&mut self, value: f64) {
		self.controls.volume.send(value).ok();
	}

	pub fn close(&mut self) {
		self.controls.close.send(true).ok();
	}
}

#[derive(Debug, Default, Baton)]
struct Controls {
	volume: f64,
	media: Option<MediaStream>,
	close: bool,
}

#[derive(Debug, Default, Baton)]
struct Status {
	connected: bool,
	error: Option<String>,
}

struct PublishBackend {
	src: Url,

	controls: ControlsRecv,
	status: StatusSend,
}

impl PublishBackend {
	fn new(src: Url, controls: ControlsRecv, status: StatusSend) -> Self {
		Self { src, controls, status }
	}

	async fn run(&mut self) -> Result<()> {
		let session = super::session::connect(&self.src).await?;
		let path: moq_transfork::Path = self.src.path_segments().ok_or(Error::InvalidUrl)?.collect();
		let mut active = None;

		tracing::info!(%self.src, "connected");

		self.status.connected.send(true).ok();

		loop {
			tokio::select! {
				Some(true) = self.controls.close.recv() => return Ok(()),
				Some(media) = self.controls.media.recv() => {
					active.take(); // Close the previous broadcast

					if let Some(media) = media {
						let mut broadcast = moq_karp::BroadcastProducer::new(session.clone(), path.clone())?;
						Self::init(&mut broadcast, media).await?;
						active = Some(broadcast);
					}
				},
				else => return Ok(()),
			}
		}
	}

	async fn init(broadcast: &mut moq_karp::BroadcastProducer, media: &MediaStream) -> Result<()> {
		// TODO listen for updates
		for track in media.get_tracks() {
			// TODO wrap so we call `stop` on Drop
			let track: web_sys::MediaStreamTrack = track.unchecked_into();

			match track.kind().as_str() {
				"audio" => {
					tracing::warn!("skipping audio track");
				}
				"video" => {
					let encoder_config = Self::video_config(&track).await?;
					let (encoder, encoded) = encoder_config.clone().init()?;

					let name = track.id();

					// Unfortunately... we don't know the description right now.
					// We actually need to pump the encoder at least once.
					let init = web_sys::MediaStreamTrackProcessorInit::new(&track);
					let processor = web_sys::MediaStreamTrackProcessor::new(&init).unwrap();
					let mut reader: web_streams::Reader<web_sys::VideoFrame> =
						web_streams::Reader::new(&processor.readable())?;

					// Pump the encoder to get the first decoded frame, so we can learn the description.
					// TODO helper function
					let decoder_config;
					loop {
						tokio::select! {
							Some(frame) = reader.read().transpose() => encoder.encode(frame?.into())?,
							config = encoded.config() => {
								decoder_config = Some(config?);
								break;
							},
							else => return Err(Error::CaptureFailed),
						}
					}
					let decoder_config = decoder_config.unwrap();

					let frame = reader.read().await?.ok_or(Error::CaptureFailed)?;
					encoder.encode(frame.into())?;

					let track = moq_karp::Video {
						track: moq_karp::Track { name, priority: 2 },
						codec: encoder_config.codec.try_into().unwrap(),
						description: decoder_config.description,
						resolution: moq_karp::Dimensions {
							width: encoder_config.resolution.width,
							height: encoder_config.resolution.height,
						},
						bitrate: encoder_config.bit_rate,
					};

					broadcast.publish_video(track)?;
				}
				_ => {
					tracing::warn!("unknown track kind: {:?}", track.kind());
				}
			}
		}

		Ok(())
	}

	async fn video_config(track: &web_sys::MediaStreamTrack) -> Result<web_codecs::VideoEncoderConfig> {
		let settings = track.get_settings();

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

		// A list of codecs and profiles sorted in preferred order.
		// TODO automate this list by looping over profile/level pairs
		const CODECS: &[&'static str] = &[
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
		let accelerated = web_codecs::VideoEncoderConfig {
			hardware_acceleration: Some(true),
			..base.clone()
		};

		for codec in CODECS {
			let config = web_codecs::VideoEncoderConfig {
				codec: codec.to_string(),
				..accelerated.clone()
			};

			if config.is_supported().await? {
				return Ok(config);
			}
		}

		// All accelerated configurations failed, try software encoding.
		for codec in CODECS {
			let config = web_codecs::VideoEncoderConfig {
				codec: codec.to_string(),
				..base.clone()
			};

			if config.is_supported().await? {
				return Ok(config);
			}
		}

		Err(Error::Unsupported)
	}
}
