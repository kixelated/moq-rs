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
			publish: Publish::default(),
			watch: Watch::default(),
			bridge,
		}
	}

	pub async fn run(&mut self) {
		loop {
			tokio::select! {
				Some(session) = async { Some(self.connecting.as_mut()?.established().await) } => {
					let connecting = self.connecting.take().unwrap();

					let session = match session {
						Ok(session) => session,
						Err(err) => {
							tracing::error!(?err, "failed to establish connection");
							continue;
						}
					};

					tracing::info!("connected to server");

					let path = connecting.path;
					let room = hang::Room::new(session, path);

					self.watch.set_room(Some(room.clone()));
					self.publish.set_room(Some(room));
				}
				// `if false` is very weird.
				// Basically tokio will poll the future, but ignore the result based on the (optional) conditional.
				// This is extremely confusing because you would expect it to check the conditional first.
				// An alternative would be to make `run()` block forever but that's no fun.
				_ = self.watch.run(), if false => unreachable!(),
				_ = self.publish.run(), if false => unreachable!(),
				command = self.bridge.recv() => {
					if let Err(err) = self.recv_command(command).await {
						tracing::error!(?err, "failed to process command");
					}
				}
			}
		}
	}

	pub async fn recv_command(&mut self, cmd: RoomCommand) -> Result<()> {
		tracing::info!(?cmd, "received command");

		match cmd {
			RoomCommand::Connect { url } => {
				tracing::info!(?url, "connecting");
				if let Some(url) = url {
					self.connecting = Some(Connect::new(url));
				} else {
					self.connecting = None;
				}

				self.watch.set_room(None);
				self.publish.set_room(None);
			}
			RoomCommand::Watch(cmd) => {
				self.watch.recv_command(cmd);
			}
			RoomCommand::Publish(cmd) => self.publish.recv_command(cmd).await?,
		}

		Ok(())
	}
}
