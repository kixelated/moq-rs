use std::time::Duration;

use baton::Baton;
use moq_karp::{moq_transfork::Path, BroadcastConsumer};
use url::Url;
use wasm_bindgen_futures::spawn_local;

use super::{Renderer, Video, WatchState};
use crate::{Connect, Error, Result};

#[derive(Debug, Baton)]
pub struct Controls {
	pub url: Option<Url>,
	pub paused: bool,
	pub volume: f64,
	pub canvas: Option<web_sys::OffscreenCanvas>,

	// Play media faster until this latency is reached.
	pub latency: Duration,

	// Drop media if the latency exceeds this value.
	pub latency_max: Duration,
}

impl Default for Controls {
	fn default() -> Self {
		Self {
			url: None,
			paused: false,
			volume: 1.0,
			canvas: None,
			latency: Duration::ZERO,
			latency_max: Duration::from_secs(10),
		}
	}
}

#[derive(Debug, Default, Baton)]
pub struct Status {
	pub state: WatchState,
	pub error: Option<Error>,
}

pub struct Backend {
	controls: ControlsRecv,
	status: StatusSend,

	path: Path,
	connect: Option<Connect>,
	broadcast: Option<BroadcastConsumer>,
	video: Option<Video>,

	renderer: Renderer,
}

impl Backend {
	pub fn new(controls: ControlsRecv, status: StatusSend) -> Self {
		Self {
			controls,
			status,

			path: Path::default(),
			connect: None,

			broadcast: None,
			video: None,
			renderer: Renderer::new(),
		}
	}

	pub fn start(mut self) {
		spawn_local(async move {
			if let Err(err) = self.run().await {
				self.status.error.set(Some(err));
			}

			self.status.state.set(WatchState::Error);
		});
	}

	async fn run(&mut self) -> Result<()> {
		loop {
			tokio::select! {
				url = self.controls.url.next() => {
					let url = url.ok_or(Error::Closed)?;

					self.broadcast = None;
					self.video = None;

					if let Some(url) = url {
						// Connect using the base of the URL.
						let mut addr = url.clone();
						addr.set_fragment(None);
						addr.set_query(None);
						addr.set_path("");

						self.path = url.path_segments().ok_or(Error::InvalidUrl(url.to_string()))?.collect();
						self.connect = Some(Connect::new(addr));

						self.status.state.set(WatchState::Connecting);
					} else {
						self.path = Path::default();
						self.connect = None;

						self.status.state.set(WatchState::Idle);
					}
				},
				Some(session) = async { Some(self.connect.as_mut()?.established().await) } => {
					tracing::info!("connected");
					let broadcast = moq_karp::BroadcastConsumer::new(session?, self.path.clone());
					self.status.state.set(WatchState::Connected);

					self.broadcast = Some(broadcast);
					self.connect = None;
				},
				Some(catalog) = async { Some(self.broadcast.as_mut()?.catalog().await) } => {
					let catalog = match catalog? {
						Some(catalog) => catalog,
						None => {
							// There's no catalog, so the stream is offline.
							// Note: We keep trying because the stream might come online later.
							self.status.state.set(WatchState::Offline);
							self.video = None;
							continue;
						},
					};

					// NOTE: We fire this event every time the catalog changes.
					self.status.state.set(WatchState::Live);

					// TODO add an ABR module
					if let Some(info) = catalog.video.first() {
						let mut track = self.broadcast.as_mut().unwrap().track(&info.track)?;
						track.set_latency(self.controls.latency.get());
						self.renderer.set_resolution(info.resolution);

						let video = Video::new(track, info.clone())?;
						self.video = Some(video);
					} else {
						self.renderer.set_resolution(Default::default());
						self.video = None;
					}

				},
				Some(frame) = async { self.video.as_mut()?.frame().await.transpose() } => {
					self.renderer.push(frame?);
				},
				canvas = self.controls.canvas.next() => {
					self.renderer.set_canvas(canvas.ok_or(Error::Closed)?.clone());
				},
				// TODO temporarily unsubscribe on pause
				paused = self.controls.paused.next() => {
					self.renderer.set_paused(paused.ok_or(Error::Closed)?);
				},
				latency = self.controls.latency.next() => {
					let latency = latency.ok_or(Error::Closed)?;
					self.renderer.set_latency(latency);
					self.video.as_mut().map(|v| v.track.set_latency(latency));
				},
				latency_max = self.controls.latency_max.next() => {
					self.renderer.set_latency_max(latency_max.ok_or(Error::Closed)?);
				},
				else => return Ok(()),
			}
		}
	}
}
