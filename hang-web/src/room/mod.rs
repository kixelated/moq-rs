mod command;
mod connect;
mod status;

pub use command::*;
pub use connect::*;
pub use status::*;

use crate::{Bridge, Publish, Result, Watch};

pub struct Room {
	bridge: Bridge,
	connecting: Option<Connect>,
	publish: Publish,
	watch: Watch,
}

impl Room {
	pub fn new(bridge: Bridge) -> Self {
		Self {
			connecting: None,
			publish: Publish::new(),
			watch: Watch::default(),
			bridge,
		}
	}

	pub async fn run(&mut self) -> Result<()> {
		loop {
			tokio::select! {
				Some(session) = async { Some(self.connecting.as_mut()?.established().await) } => {
					tracing::info!("connected to server");
					let session = session?;
					let path = self.connecting.take().unwrap().path;
					let room = hang::Room::new(session, path);

					self.watch.set_room(Some(room.clone()));
					self.publish.set_room(Some(room))?;
				},
				Err(err) = self.watch.run() => return Err(err),
				Err(err) = self.publish.run() => return Err(err),
				Some(command) = self.bridge.recv() => self.recv_command(command).await?,
				else => return Ok(()),
			}
		}
	}

	pub async fn recv_command(&mut self, command: RoomCommand) -> Result<()> {
		tracing::info!(?command, "received command");

		match command {
			RoomCommand::Connect { url } => {
				tracing::info!(?url, "connecting");
				if let Some(url) = url {
					self.connecting = Some(Connect::new(url));
				} else {
					self.connecting = None;
				}

				self.watch.set_room(None);
				self.publish.set_room(None)?;
			}
			RoomCommand::Watch(command) => {
				self.watch.recv_command(command);
			}
			RoomCommand::Publish(command) => {
				self.publish.recv_command(command).await?;
			}
		}

		Ok(())
	}
}
