mod audio;
mod message;
mod video;

use std::time::Duration;

pub use audio::*;
pub use message::*;
pub use video::*;

use crate::{Connect, Result};
use hang::moq_lite;
use moq_lite::Session;

#[derive(Default)]
pub struct Watch {
	connect: Option<Connect>,

	broadcast: Option<hang::BroadcastConsumer>,
	catalog: Option<hang::Catalog>,

	audio: Audio,
	video: Video,
}

impl Watch {
	pub fn recv(&mut self, command: WatchCommand) -> Result<()> {
		match command {
			WatchCommand::Connect(url) => {
				self.connect = None;

				if let Some(url) = url {
					self.connect = Some(Connect::new(url));
				}
			}
			WatchCommand::Canvas(canvas) => {
				self.video.set_canvas(canvas);
				self.init_video();
			}
			WatchCommand::Worklet { port, sample_rate } => {
				self.audio.set_worklet(port);
				self.audio.set_sample_rate(sample_rate);
				self.init_audio();
			}
			WatchCommand::Latency(latency) => self.video.set_latency(Duration::from_millis(latency.into())),
			WatchCommand::Paused(paused) => self.video.set_paused(paused),
			WatchCommand::Visible(visible) => {
				self.video.set_visible(visible);
				self.init_video();
			}
		};

		Ok(())
	}

	pub async fn run(&mut self) -> Result<()> {
		loop {
			tokio::select! {
				Some(session) = async { Some(self.connect.as_mut()?.established().await) } => {
					let connect = self.connect.take().unwrap();
					self.connected(connect, session?)?;
				}
				Some(res) = async { Some(self.broadcast.as_mut()?.catalog.next().await) } => {
					self.catalog = res?;
					tracing::info!(catalog = ?self.catalog, "catalog updated");
					self.init_audio();
					self.init_video();
				}
				Err(err) = self.audio.run() => return Err(err),
				Err(err) = self.video.run() => return Err(err),
				// Return Ok() when there's nothing to do.
				else => return Ok(()),
			}
		}
	}

	fn connected(&mut self, connect: Connect, session: Session) -> Result<()> {
		tracing::info!("connected to server");

		let broadcast = connect.path.strip_prefix("/").unwrap();
		let broadcast = session.namespace(broadcast.into());
		self.broadcast = Some(broadcast.into());

		Ok(())
	}

	fn init_audio(&mut self) {
		match self.audio.init(self.broadcast.as_ref(), self.catalog.as_ref()) {
			Ok(_) => tracing::info!("audio initialized"),
			Err(err) => tracing::warn!(?err, "failed to initialize audio"),
		}
	}

	fn init_video(&mut self) {
		match self.video.init(self.broadcast.as_ref(), self.catalog.as_ref()) {
			Ok(_) => tracing::info!("video initialized"),
			Err(err) => tracing::warn!(?err, "failed to initialize video"),
		}
	}
}
