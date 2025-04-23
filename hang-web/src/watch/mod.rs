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
			WatchCommand::Canvas(canvas) => self.video.set_canvas(canvas),
			WatchCommand::Worklet { port, sample_rate } => self.audio.set_worklet(port, sample_rate),
			WatchCommand::Latency(latency) => self.video.set_latency(Duration::from_millis(latency.into())),
			WatchCommand::Paused(paused) => self.video.set_paused(paused),
			WatchCommand::Visible(visible) => self.video.set_visible(visible),
			WatchCommand::Muted(muted) => self.audio.set_muted(muted),
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
					let catalog = res?;
					self.audio.set_catalog(self.broadcast.clone(), catalog.clone());
					self.video.set_catalog(self.broadcast.clone(), catalog.clone());

					match catalog {
						Some(catalog) => tracing::info!(?catalog, "catalog updated"),
						None => {
							tracing::info!("broadcast ended");
							self.broadcast.take();
						}
					}
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
}
