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
	_status: StatusRecv,
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
			_status: status.1,
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

		let id = js_sys::Date::now() as u64;
		let broadcast = moq_karp::BroadcastProducer::new(session.clone(), path.clone(), id)?;

		tracing::info!(%self.src, ?broadcast, "connected");

		self.status.connected.send(true).ok();

		loop {
			tokio::select! {
				Some(true) = self.controls.close.recv() => return Ok(()),
				Some(media) = self.controls.media.recv() => {
					active.take();

					if let Some(media) = media {
						let media = PublishMedia::new(broadcast.clone(), media.clone());
						active = Some(media);
					}
				},
				else => return Ok(()),
			}
		}
	}
}

struct PublishMedia {
	_audio: Vec<PublishAudio>,
	_video: Vec<PublishVideo>,
}

impl PublishMedia {
	fn new(broadcast: moq_karp::BroadcastProducer, media: MediaStream) -> Self {
		let _audio = Vec::new();
		let mut _video = Vec::new();

		// TODO listen for updates
		for track in media.get_tracks() {
			let track: web_sys::MediaStreamTrack = track.unchecked_into();

			match track.kind().as_str() {
				"audio" => {
					tracing::warn!("skipping audio track");
				}
				"video" => {
					_video.push(PublishVideo::new(broadcast.clone(), track));
				}
				_ => {
					tracing::warn!("unknown track kind: {:?}", track.kind());
				}
			}
		}

		Self { _audio, _video }
	}
}

#[derive(Clone)]
struct PublishVideo {
	broadcast: moq_karp::BroadcastProducer,
	track: web_sys::MediaStreamTrack,
}

impl PublishVideo {
	fn new(broadcast: moq_karp::BroadcastProducer, track: web_sys::MediaStreamTrack) -> Self {
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
		let (encoder, mut encoded) = config.clone().init()?;

		let name = self.track.id();

		// Unfortunately... we don't know the description right now.
		// We actually need to pump the encoder at least once.
		let init = web_sys::MediaStreamTrackProcessorInit::new(&self.track);
		let processor = web_sys::MediaStreamTrackProcessor::new(&init).unwrap();
		let mut reader: web_streams::Reader<web_sys::VideoFrame> = web_streams::Reader::new(&processor.readable())?;

		// Pump the encoder to get the first decoded frame, so we can learn the description.
		// TODO helper function
		let decoder_config;
		loop {
			tokio::select! {
				Some(frame) = reader.read().transpose() => encoder.encode(frame?.into())?,
				Some(config) = encoded.config() => {
					decoder_config = config;
					break;
				},
				else => return Err(Error::CaptureFailed),
			}
		}

		let frame = reader.read().await?.ok_or(Error::CaptureFailed)?;
		encoder.encode(frame.into())?;

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

		let mut track = self.broadcast.publish_video(track)?;

		while let Some(frame) = encoded.frame().await? {
			track.write(moq_karp::Frame {
				timestamp: moq_karp::Timestamp::from_micros(frame.timestamp as _),
				keyframe: frame.keyframe,
				payload: frame.payload,
			});
		}

		Ok(())
	}

	async fn config(&self) -> Result<web_codecs::VideoEncoderConfig> {
		let settings = self.track.get_settings();

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

impl Drop for PublishVideo {
	fn drop(&mut self) {
		// Terminate when one of the handles is dropped.
		self.track.stop();
	}
}

struct PublishAudio {}
