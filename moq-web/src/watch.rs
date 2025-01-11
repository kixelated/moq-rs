use url::Url;
use wasm_bindgen::prelude::*;

use baton::Baton;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlCanvasElement;

use crate::{Decoder, Error, Renderer, Result};

#[derive(Debug, Default, Baton)]
struct Controls {
	pub url: Option<Url>,
	pub paused: bool,
	pub volume: f64,
	pub canvas: Option<HtmlCanvasElement>,
	pub close: bool,
}

#[derive(Debug, Default, Copy, Clone)]
#[wasm_bindgen]
pub enum WatchState {
	#[default]
	Init,
	Connecting,
	Connected,
	Offline,
	Active,
	Error,
}

#[derive(Debug, Default, Baton)]
struct Status {
	pub state: WatchState,
	pub error: Option<String>,
}

#[wasm_bindgen]
pub struct Watch {
	controls: ControlsSend,
	status: StatusRecv,
}

#[wasm_bindgen]
impl Watch {
	#[wasm_bindgen(constructor)]
	pub fn new() -> Self {
		let controls = Controls::default().baton();
		let status = Status::default().baton();
		let mut backend = WatchBackend::new(controls.1, status.0);

		spawn_local(async move {
			if let Err(err) = backend.run().await {
				tracing::error!(?err, "backend error");
			}
		});

		Self {
			controls: controls.0,
			status: status.1,
		}
	}

	pub fn load(&mut self, url: &str) -> Result<()> {
		let url = Url::parse(url).map_err(|_| Error::InvalidUrl)?;
		self.controls.url.send(Some(url)).map_err(|_| Error::Closed)
	}

	pub fn render(&mut self, canvas: Option<HtmlCanvasElement>) -> Result<()> {
		self.controls.canvas.send(canvas).map_err(|_| Error::Closed)
	}

	pub fn pause(&mut self, paused: bool) -> Result<()> {
		self.controls.paused.send(paused).map_err(|_| Error::Closed)
	}

	pub fn volume(&mut self, value: f64) -> Result<()> {
		self.controls.volume.send(value).map_err(|_| Error::Closed)
	}

	pub fn close(&mut self) -> Result<()> {
		self.controls.close.send(true).map_err(|_| Error::Closed)
	}

	pub async fn state(&mut self) -> WatchState {
		self.status.state.recv().await.unwrap_or(WatchState::Error)
	}
}

struct WatchBackend {
	controls: ControlsRecv,
	status: StatusSend,

	broadcast: Option<moq_karp::BroadcastConsumer>,
	catalog: Option<moq_karp::Catalog>,
	decoder: Option<Decoder>,
	renderer: Option<Renderer>,
	canvas: Option<HtmlCanvasElement>,
}

impl WatchBackend {
	fn new(controls: ControlsRecv, status: StatusSend) -> Self {
		Self {
			controls,
			status,

			broadcast: None,
			catalog: None,
			decoder: None,
			renderer: None,
			canvas: None,
		}
	}

	async fn run(&mut self) -> Result<()> {
		loop {
			let decoder = self.decoder.as_mut();
			let renderer = self.renderer.as_mut();
			let broadcast = self.broadcast.as_mut();

			tokio::select! {
				Some(Some(url)) = self.controls.url.recv() => {
					let session = super::session::connect(&url).await?;
					let path = url.path_segments().ok_or(Error::InvalidUrl)?.collect();
					self.broadcast = Some(moq_karp::BroadcastConsumer::new(session, path));
					self.catalog = None;
					self.decoder = None;
					self.renderer = None;

					tracing::info!(%url, "connected");
					self.status.state.send(WatchState::Connected).ok();
				}
				Some(catalog) = async { broadcast?.catalog().await.transpose() } => {
					self.catalog = Some(catalog?);
					self.init()?;
				}
				Some(Err(err)) = async move { Some(decoder?.run().await) } => return Err(err),
				Some(Err(err)) = async move { Some(renderer?.run().await) } => return Err(err),
				Some(true) = self.controls.close.recv() => return Ok(()),
				Some(canvas) = self.controls.canvas.recv() => {
					if let Some(renderer) = self.renderer.as_mut() {
						renderer.canvas(canvas.clone());
					}
					self.canvas = canvas;
				},
				else => return Ok(()),
			}
		}
	}

	fn init(&mut self) -> Result<()> {
		let broadcast = self.broadcast.as_mut().unwrap();
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
			let mut renderer = Renderer::new(decoded);
			if let Some(canvas) = self.canvas.as_mut() {
				renderer.canvas(Some(canvas.clone()));
			}

			self.decoder = Some(decoder);
			self.renderer = Some(renderer);
		} else {
			self.decoder = None;
			self.renderer = None;
		}

		Ok(())
	}
}
