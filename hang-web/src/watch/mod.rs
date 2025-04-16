mod command;
mod renderer;
mod video;

pub use command::*;
pub use renderer::*;
pub use video::*;

#[derive(Default)]
pub struct Watch {
	room: Option<hang::Room>,
	canvas: Option<web_sys::OffscreenCanvas>,
	latency: u32,
}

impl Watch {
	pub async fn run(&mut self) {
		loop {
			tokio::select! {
				Some(broadcast) = async { self.room.as_mut()?.joined().await } => {
					tracing::info!(broadcast = ?broadcast.inner.info, "joined");
					// TODO Add broadcast
				}
				else => return,
			}
		}
	}

	pub fn set_room(&mut self, room: Option<hang::Room>) {
		self.room = room;
	}

	pub fn recv_command(&mut self, command: WatchCommand) {
		match command {
			WatchCommand::Canvas(canvas) => self.canvas = canvas,
			WatchCommand::Latency(latency) => self.latency = latency,
		}
	}
}
