use ts_rs::TS;
use web_message::Message;

use crate::Result;

#[derive(Default)]
pub struct Watch {
	room: Option<hang::Room>,
	canvas: Option<web_sys::OffscreenCanvas>,
	latency: u64,
}

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/rpc.ts")]
pub enum WatchCommand {
	// Render the video to the given canvas if visible.
	#[ts(type = "OffscreenCanvas | null")]
	Canvas(Option<web_sys::OffscreenCanvas>),

	// Use the given latency for the video.
	Latency(u64),
}

impl Watch {
	pub fn new() -> Self {
		Self {
			room: None,
			canvas: None,
			latency: 0,
		}
	}

	pub async fn run(&mut self) -> Result<()> {
		loop {
			tokio::select! {
				Some(_broadcast) = async { self.room.as_mut()?.joined().await } => {
					// TODO Add broadcast
				}
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
