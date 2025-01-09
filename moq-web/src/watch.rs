use moq_karp::BroadcastConsumer;

use url::Url;
use wasm_bindgen::prelude::*;

use baton::Baton;
use wasm_bindgen_futures::spawn_local;
use web_sys::OffscreenCanvas;

use crate::{Decoder, Error, Renderer, Result};

#[derive(Debug, Default, Baton)]
struct Controls {
	pub paused: bool,
	pub volume: f64,
	pub canvas: Option<OffscreenCanvas>,
	pub close: bool,
}

#[derive(Debug, Default, Baton)]
struct Status {
	pub connected: bool,
	pub error: Option<String>,
}

#[wasm_bindgen]
pub struct Watch {
	controls: ControlsSend,
	_status: StatusRecv,
}

#[wasm_bindgen]
impl Watch {
	#[wasm_bindgen(constructor)]
	pub fn new(src: &str) -> Result<Self> {
		tracing::info!("watching: {:?}", src);
		let src = Url::parse(src).map_err(|_| Error::InvalidUrl)?;

		let controls = Controls::default().baton();
		let status = Status::default().baton();
		let mut backend = WatchBackend::new(src, controls.1, status.0);

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

	pub fn render(&mut self, canvas: Option<OffscreenCanvas>) {
		self.controls.canvas.send(canvas).ok();
	}

	pub fn pause(&mut self, paused: bool) {
		self.controls.paused.send(paused).ok();
	}

	pub fn volume(&mut self, value: f64) {
		self.controls.volume.send(value).ok();
	}

	pub fn close(&mut self) {
		self.controls.close.send(true).ok();
	}
}

struct WatchBackend {
	src: Url,

	controls: ControlsRecv,
	status: StatusSend,

	catalog: Option<moq_karp::Catalog>,
	decoder: Option<Decoder>,
	renderer: Option<Renderer>,
}

impl WatchBackend {
	fn new(src: Url, controls: ControlsRecv, status: StatusSend) -> Self {
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

		self.status.connected.send(true).ok();

		loop {
			let decoder = self.decoder.as_mut();
			let renderer = self.renderer.as_mut();

			tokio::select! {
				Some(catalog) = async { broadcast.catalog().await.transpose() } => {
					self.catalog = Some(catalog?);
					self.init(&mut broadcast)?;
				}
				Some(Err(err)) = async move { Some(decoder?.run().await) } => return Err(err),
				Some(Err(err)) = async move { Some(renderer?.run().await) } => return Err(err),
				Some(true) = self.controls.close.recv() => return Ok(()),
				Some(canvas) = self.controls.canvas.recv() => {
					if let Some(renderer) = &mut self.renderer {
						renderer.update(canvas.clone());
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

			// Construct the video decoder
			let (decoder, decoded) = web_codecs::VideoDecoderConfig {
				codec: video.codec.to_string(),
				description: video.description.clone(),
				resolution: Some(web_codecs::Dimensions {
					width: video.resolution.width,
					height: video.resolution.height,
				}),
				latency_optimized: Some(true),
				..Default::default()
			}
			.build()?;

			let track = broadcast.track(&video.track)?;

			let decoder = Decoder::new(track, decoder);
			let renderer = Renderer::new(decoded, self.controls.canvas.latest().clone());

			self.decoder = Some(decoder);
			self.renderer = Some(renderer);
		} else {
			self.decoder = None;
			self.renderer = None;
		}

		Ok(())
	}
}
