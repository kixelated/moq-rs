use moq_karp::{moq_transfork::Path, BroadcastConsumer};

use super::{ControlsConsumer, Renderer, StatusProducer, Video};
use crate::{Error, Result, Session};

pub struct Backend {
	controls: ControlsConsumer,
	status: StatusProducer,

	path: Path,
	session: Option<Session>,
	broadcast: Option<BroadcastConsumer>,
	video: Option<Video>,

	renderer: Renderer,
}

impl Backend {
	pub fn new(controls: ControlsConsumer, status: StatusProducer) -> Self {
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
		tracing::info!("running backend");

		loop {
			let session = self.session.as_mut();
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
					self.session = Some(Session::new(addr));
				},
				Some(session) = async move { Some(session?.connect().await) } => {
					let broadcast = moq_karp::BroadcastConsumer::new(session?, self.path.clone());
					self.broadcast = Some(broadcast);
					self.session = None;
				},
				Some(catalog) = async move { broadcast?.catalog().await.transpose() } => {
					let catalog = catalog?;

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
					self.renderer.set_canvas(canvas.clone());
				},
				else => return Ok(()),
			}
		}
	}
}
