use ts_rs::TS;
use web_message::Message;

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/watch/command.ts")]
pub enum WatchCommand {
	// Render the video to the given canvas if visible.
	#[ts(type = "OffscreenCanvas | null")]
	Canvas(Option<web_sys::OffscreenCanvas>),

	// Use the given latency for the video.
	Latency(u32),
}
