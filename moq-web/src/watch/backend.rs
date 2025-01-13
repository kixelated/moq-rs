use moq_karp::{moq_transfork::Path, BroadcastConsumer};

use super::{ControlsRecv, Renderer, StatusSend, Video};
use crate::{Error, Result, Session};

pub struct Backend {
	controls: ControlsRecv,
	status: StatusSend,

	path: Path,
	session: Option<Session>,
	broadcast: Option<BroadcastConsumer>,
	video: Option<Video>,

	renderer: Renderer,
}

impl Backend {
	pub fn new(controls: ControlsRecv, status: StatusSend) -> Self {
		let renderer = Renderer::new();

		Self {
			controls,
			status,

			path: Path::default(),
			session: None,

			broadcast: None,
			video: None,
			renderer,
		}
	}

	pub async fn run(&mut self) -> Result<()> {
		loop {
			let session = self.session.as_mut();
			let broadcast = self.broadcast.as_mut();
			let video = self.video.as_mut();

			tokio::select! {
				Some(Some(url)) = self.controls.url.recv() => {
					// Connect using the base of the URL.
					let mut addr = url.clone();
					addr.set_fragment(None);
					addr.set_query(None);
					addr.set_path("");

					self.path = url.path_segments().ok_or(Error::InvalidUrl)?.collect();
					self.session = Some(Session::new(addr));
				},
				Some(session) = async move { Some(session?.connect().await) } => {
					let broadcast = moq_karp::BroadcastConsumer::new(session?, self.path.clone());
					self.broadcast = Some(broadcast);
				},
				Some(catalog) = async move { broadcast?.catalog().await.transpose() } => {
					let catalog = catalog?;

					// TODO add an ABR module
					if let Some(video) = catalog.video.first() {
						let consumer = self.broadcast.as_mut().unwrap().track(&video.track)?;
						let video = Video::new(video.clone(), consumer)?;
						self.video = Some(video);
					} else {
						self.video = None;
					}
				},
				Some(frame) = async move { video?.frame().await.transpose() } => {
					let frame = frame?;
					self.renderer.render(frame);
				},
				Some(true) = self.controls.close.recv() => return Ok(()),
				Some(canvas) = self.controls.canvas.recv() => self.renderer.set_canvas(canvas.clone()),
				else => return Ok(()),
			}
		}
	}
}
