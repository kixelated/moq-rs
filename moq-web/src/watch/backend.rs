use baton::Baton;
use moq_karp::{moq_transfork::Path, BroadcastConsumer};
use url::Url;
use wasm_bindgen_futures::spawn_local;

use super::{Renderer, Video, WatchState};
use crate::{Connect, Error, Result};

#[derive(Debug, Default, Baton)]
pub struct Controls {
	pub url: Option<Url>,
	pub paused: bool,
	pub volume: f64,
	pub canvas: Option<web_sys::OffscreenCanvas>,
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
				tracing::error!(?err, "backend error");
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

						self.path = url.path_segments().ok_or(Error::InvalidUrl)?.collect();
						self.connect = Some(Connect::new(addr));

						self.status.state.set(WatchState::Connecting);
					} else {
						self.path = Path::default();
						self.connect = None;

						self.status.state.set(WatchState::Idle);
					}
				},
				Some(session) = async { Some(self.connect.as_mut()?.established().await) } => {
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
					self.status.state.set(WatchState::Playing);

					// TODO add an ABR module
					if let Some(info) = catalog.video.first() {
						let track = self.broadcast.as_mut().unwrap().track(&info.track)?;
						let video = Video::new(track, info.clone())?;
						self.video = Some(video);
					} else {
						self.video = None;
					}
				},
				Some(frame) = async { self.video.as_mut()?.frame().await.transpose() } => {
					let frame = frame?;
					self.renderer.render(frame);
				},
				canvas = self.controls.canvas.next() => {
					let canvas = canvas.ok_or(Error::Closed)?;
					self.renderer.canvas(canvas.clone());
				},
				else => return Ok(()),
			}
		}
	}
}
