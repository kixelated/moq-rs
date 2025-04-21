use ts_rs::TS;
use url::Url;
use web_message::Message;

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/watch/message.ts")]
pub enum WatchCommand {
	// Join a room at the given URL, or none to leave the current room.
	Connect(Option<Url>),

	// Render the video to the given canvas, or none to disable rendering.
	// NOTE: You can only transfer a canvas once; use Visible to show/hide the video.
	#[ts(type = "OffscreenCanvas | null")]
	Canvas(Option<web_sys::OffscreenCanvas>),

	// Set the worklet port so we can send audio data to it.
	#[ts(type = "MessagePort | null")]
	Worklet(Option<web_sys::MessagePort>),

	// Set the latency of the video.
	// Default: 0
	Latency(u32),

	// Pause or resume the video.
	// Default: false
	Paused(bool),

	// Set the visibility of the video.
	// Default: true
	Visible(bool),
}
