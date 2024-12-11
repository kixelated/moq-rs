use moq_karp::BroadcastConsumer;
use tokio::sync::watch;

use url::Url;
use wasm_bindgen::prelude::*;

use wasm_bindgen_futures::spawn_local;
use web_sys::OffscreenCanvas;

use crate::{Decoder, Error, Renderer, Result, Run};

#[derive(Debug, Default)]
struct Controls {
	paused: bool,
	volume: f64,
	canvas: Option<OffscreenCanvas>,
	close: bool,
}

#[derive(Debug, Default)]
struct Status {
	connected: bool,
	error: Option<String>,
}

#[wasm_bindgen]
pub struct Watch {
	controls: watch::Sender<Controls>,
	_status: watch::Receiver<Status>,
}

#[wasm_bindgen]
impl Watch {
	#[wasm_bindgen(constructor)]
	pub fn new(src: &str) -> Result<Self> {
		let src = Url::parse(src).map_err(|_| Error::InvalidUrl)?;

		let controls = watch::channel(Controls::default());
		let status = watch::channel(Status::default());
		let mut backend = WatchBackend::new(src, controls.1, status.0.clone());

		spawn_local(async move {
			if let Err(err) = backend.run().await {
				tracing::error!(?err, "backend error");

				status.0.send_modify(|status| {
					status.error = err.to_string().into();
				});
			} else {
				tracing::warn!("backend closed");
			}
		});

		Ok(Self {
			controls: controls.0,
			_status: status.1,
		})
	}

	pub fn render(&mut self, canvas: Option<OffscreenCanvas>) {
		self.controls.send_modify(|controls| {
			controls.canvas = canvas;
		});
	}

	pub fn pause(&mut self, paused: bool) {
		self.controls.send_modify(|controls| {
			controls.paused = paused;
		});
	}

	pub fn volume(&mut self, value: f64) {
		self.controls.send_modify(|controls| {
			controls.volume = value;
		});
	}

	pub fn close(&mut self) {
		self.controls.send_modify(|controls| {
			controls.close = true;
		});
	}
}

struct WatchBackend {
	src: Url,

	controls: watch::Receiver<Controls>,
	status: watch::Sender<Status>,

	catalog: Option<moq_karp::Catalog>,
	decoder: Option<Decoder>,
	renderer: Option<Renderer>,
}

impl WatchBackend {
	fn new(src: Url, controls: watch::Receiver<Controls>, status: watch::Sender<Status>) -> Self {
		Self {
			src,
			controls,
			status,

			catalog: None,
			decoder: None,
			renderer: None,
		}
	}

	async fn run(&mut self) -> Result<()> {
		let session = super::session::connect(&self.src).await?;
		let path = self.src.path_segments().ok_or(Error::InvalidUrl)?.collect();
		let mut broadcast = moq_karp::BroadcastConsumer::new(session, path);

		tracing::info!(%self.src, "connected");

		self.status.send_modify(|status| {
			status.connected = true;
		});

		loop {
			tokio::select! {
				Some(catalog) = async { broadcast.catalog().await.transpose() } => {
					self.catalog = Some(catalog?);
					self.init(&mut broadcast)?;
				}
				Err(err) = self.decoder.run() => return Err(err),
				Err(err) = self.renderer.run() => return Err(err),
				changed = self.controls.changed() => {
					if changed.is_err() {
						return Ok(());
					}

					let controls = self.controls.borrow_and_update();
					if controls.close {
						return Ok(());
					}

					if let Some(renderer) = &mut self.renderer {
						renderer.update(controls.canvas.clone());
					}
				},
				else => return Ok(()),
			}
		}
	}

	fn init(&mut self, broadcast: &mut BroadcastConsumer) -> Result<()> {
		let catalog = self.catalog.as_ref().unwrap();

		tracing::info!(?catalog, "initializing");

		if let Some(video) = catalog.video.first() {
			tracing::info!("fetching video track: {:?}", video);

			let (decoder, decoded) = web_codecs::video_decoder();

			let mut config = web_codecs::VideoDecoderConfig::new(video.codec.to_string())
				.coded_dimensions(video.resolution.width as _, video.resolution.height as _)
				.latency_optimized();

			if !video.description.is_empty() {
				config = config.description(video.description.clone());
			}

			decoder.configure(&config)?;

			let track = broadcast.track(&video.track)?;
			let controls = self.controls.borrow();

			let decoder = Decoder::new(track, decoder);
			let renderer = Renderer::new(decoded, controls.canvas.clone());

			self.decoder = Some(decoder);
			self.renderer = Some(renderer);
		} else {
			self.decoder = None;
			self.renderer = None;
		}

		Ok(())
	}
}
