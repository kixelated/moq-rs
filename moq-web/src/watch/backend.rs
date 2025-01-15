use moq_karp::{moq_transfork::Path, BroadcastConsumer};
use wasm_bindgen_futures::spawn_local;

use super::{ControlsRecv, Renderer, StatusSend, Video, WatchState};
use crate::{Connect, Error, Result};

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
			let connect = self.connect.as_mut();
			let broadcast = self.broadcast.as_mut();
			let video = self.video.as_mut();

			tokio::select! {
				Some(Some(url)) = self.controls.url.next() => {
					// Connect using the base of the URL.
					let mut addr = url.clone();
					addr.set_fragment(None);
					addr.set_query(None);
					addr.set_path("");

					self.path = url.path_segments().ok_or(Error::InvalidUrl)?.collect();
					self.connect = Some(Connect::new(addr));
					self.broadcast = None;
					self.video = None;

					self.status.state.set(WatchState::Connecting);
				},
				Some(session) = async move { Some(connect?.established().await) } => {
					let broadcast = moq_karp::BroadcastConsumer::new(session?, self.path.clone());
					self.status.state.set(WatchState::Connected);

					self.broadcast = Some(broadcast);
					self.connect = None;
				},
				Some(catalog) = async move { Some(broadcast?.catalog().await) } => {
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
				Some(frame) = async move { video?.frame().await.transpose() } => {
					let frame = frame?;
					self.renderer.render(frame);
				},
				Some(true) = self.controls.close.next() => {
					return Ok(());
				},
				Some(canvas) = self.controls.canvas.next() => {
					self.renderer.canvas(canvas.clone());
				},
				else => return Ok(()),
			}
		}
	}
}
