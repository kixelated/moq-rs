mod command;
mod renderer;
mod video;

pub use command::*;
pub use renderer::*;
pub use video::*;

use crate::{Connect, Result};
use hang::moq_lite::Session;

#[derive(Default)]
pub struct Watch {
	connect: Option<Connect>,
	room: Option<hang::Room>,
	canvas: Option<web_sys::OffscreenCanvas>,
	latency: u32,
}

impl Watch {
	pub fn recv(&mut self, command: WatchCommand) -> Result<()> {
		match command {
			WatchCommand::Connect(url) => {
				self.connect = None;

				if let Some(url) = url {
					self.connect = Some(Connect::new(url)?);
				}
			}
			WatchCommand::Canvas(canvas) => self.canvas = canvas,
			WatchCommand::Latency(latency) => self.latency = latency,
		};

		Ok(())
	}

	fn connected(&mut self, connect: Connect, session: Session) -> Result<()> {
		tracing::info!("connected to server");

		let path = connect.path.strip_prefix("/").unwrap().to_string();
		let room = hang::Room::new(session, path.to_string());
		self.room = Some(room);

		Ok(())
	}

	pub async fn run(&mut self) -> Result<()> {
		loop {
			tokio::select! {
				Some(session) = async { Some(self.connect.as_mut()?.established().await) } => {
					let connect = self.connect.take().unwrap();
					self.connected(connect, session?)?;
				}
				Some(broadcast) = async { self.room.as_mut()?.joined().await } => {
					tracing::info!(broadcast = ?broadcast.inner.info, "joined");
					// TODO Add broadcast
				}
				else => return Ok(()),
			}
		}
	}
}
