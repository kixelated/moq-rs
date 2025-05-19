mod audio;
mod message;
mod video;

use std::time::Duration;

pub use audio::*;
pub use message::*;
pub use video::*;

use crate::{Bridge, Connect, ConnectionStatus, Result};
use hang::{moq_lite, Catalog};
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
					Bridge::send(ConnectionStatus::Connecting.into());
				} else {
					Bridge::send(ConnectionStatus::Disconnected.into());
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

	pub async fn run(&mut self) {
		loop {
			tokio::select! {
				Some(session) = async { Some(self.connect.as_mut()?.established().await) } => {
					let connect = self.connect.take().unwrap();
					let path = connect.path.strip_prefix("/").unwrap();
					self.set_session(path, session.map_err(Into::into))
				},
				Some(catalog) = async { Some(self.broadcast.as_mut()?.catalog.next().await) } => {
					let broadcast = self.broadcast.take().unwrap(); // unset just in case
					self.set_catalog(broadcast, catalog.map_err(Into::into));
				}
				// Run these in parallel but they'll never return.
				_ = self.audio.run() => {},
				_ = self.video.run() => {},
			}
		}
	}

	fn set_session(&mut self, broadcast: &str, session: Result<Session>) {
		let session = match session {
			Ok(session) => session,
			Err(err) => {
				Bridge::send(ConnectionStatus::Error(err.to_string()).into());
				return;
			}
		};

		let broadcast = session.consume(&format!("{}.hang", broadcast).into());
		self.broadcast = Some(hang::BroadcastConsumer::new(broadcast));

		Bridge::send(ConnectionStatus::Connected.into());
	}

	fn set_catalog(&mut self, broadcast: hang::BroadcastConsumer, catalog: Result<Option<Catalog>>) {
		let catalog = match catalog {
			Ok(catalog) => catalog,
			Err(err) => {
				Bridge::send(ConnectionStatus::Error(err.to_string()).into());
				return;
			}
		};

		if catalog.is_some() {
			// TODO don't send duplicate events
			Bridge::send(ConnectionStatus::Live.into());
			self.broadcast = Some(broadcast.clone());
		} else {
			Bridge::send(ConnectionStatus::Offline.into());
			self.broadcast.take();
		}

		self.audio.set_catalog(Some(broadcast.clone()), catalog.clone());
		self.video.set_catalog(Some(broadcast), catalog);
	}
}
