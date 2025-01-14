use moq_karp::moq_transfork;
use url::Url;

use super::{ControlsRecv, Media, StatusSend};
use crate::{Error, Result, Session};

pub struct Backend {
	src: Url,

	controls: ControlsRecv,
	status: StatusSend,
}

impl Backend {
	pub fn new(src: Url, controls: ControlsRecv, status: StatusSend) -> Self {
		Self { src, controls, status }
	}

	pub async fn run(&mut self) -> Result<()> {
		let mut session = Session::new(self.src.clone());
		let session = session.connect().await?;

		let path: moq_transfork::Path = self.src.path_segments().ok_or(Error::InvalidUrl)?.collect();
		let mut active = None;

		let broadcast = moq_karp::BroadcastProducer::new(session.clone(), path.clone())?;

		tracing::info!(%self.src, ?broadcast, "connected");

		self.status.connected.send(true).ok();

		loop {
			tokio::select! {
				Some(true) = self.controls.close.recv() => return Ok(()),
				Some(media) = self.controls.media.recv() => {
					active.take();

					if let Some(media) = media {
						let media = Media::new(broadcast.clone(), media.clone());
						active = Some(media);
					}
				},
				else => return Ok(()),
			}
		}
	}
}
