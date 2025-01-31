use moq_karp::BroadcastConsumer;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::spawn_local;

use super::{ControlsRecv, Renderer, StatusSend, Video};
use crate::{Connect, Error, Result};

#[derive(Debug, Default, Copy, Clone, PartialEq)]
#[wasm_bindgen]
pub enum BackendState {
	#[default]
	Idle,
	Connecting,
	Connected,
	Live,
	Offline,
}

pub struct Backend {
	controls: ControlsRecv,
	status: StatusSend,

	connect: Option<Connect>,
	path: Option<String>,
	broadcast: Option<BroadcastConsumer>,
	video: Option<Video>,

	renderer: Renderer,
}

impl Backend {
	pub fn new(controls: ControlsRecv, status: StatusSend) -> Self {
		Self {
			renderer: Renderer::new(controls.clone(), status.clone()),

			controls,
			status,

			path: None,
			connect: None,

			broadcast: None,
			video: None,
		}
	}

	pub fn start(mut self) {
		spawn_local(async move {
			if let Err(err) = self.run().await {
				self.status.error.set(Some(err));
			}
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

						self.path = Some(url.path().to_string());
						self.connect = Some(Connect::new(addr));

						self.status.backend.set(BackendState::Connecting);
					} else {
						self.path = None;
						self.connect = None;

						self.status.backend.set(BackendState::Idle);
					}
				},
				Some(session) = async { Some(self.connect.as_mut()?.established().await) } => {
					let path = self.path.as_ref().unwrap().clone();
					let broadcast = moq_karp::BroadcastConsumer::new(session?, path);
					self.status.backend.set(BackendState::Connected);

					self.broadcast = Some(broadcast);
					self.connect = None;
				},
				Some(catalog) = async { Some(self.broadcast.as_mut()?.next_catalog().await) } => {
					let catalog = match catalog? {
						Some(catalog) => catalog.clone(),
						None => {
							// There's no catalog, so the stream is offline.
							// Note: We keep trying because the stream might come online later.
							self.status.backend.set(BackendState::Offline);
							self.video = None;
							continue;
						},
					};

					// NOTE: We fire this event every time the catalog changes.
					self.status.backend.set(BackendState::Live);

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
				_ = self.controls.paused.next() => {
					// TODO temporarily unsubscribe on pause
				},
				latency = self.controls.latency.next() => {
					let latency = latency.ok_or(Error::Closed)?;
					if let Some(video) = self.video.as_mut() {
						 video.track.set_latency(latency);
					}
				},
				else => return Ok(()),
			}
		}
	}
}
