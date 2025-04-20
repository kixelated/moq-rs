use ts_rs::TS;
use web_message::Message;

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/watch/command.ts")]
pub enum WatchCommand {
	// Join a room at the given URL, or none to leave the current room.
	Connect(Option<String>),

	// Render the video to the given canvas, or none to disable rendering.
	#[ts(type = "OffscreenCanvas | null")]
	Canvas(Option<web_sys::OffscreenCanvas>),

	// Set the latency of the video.
	Latency(u32),
}
