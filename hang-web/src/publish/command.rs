use ts_rs::TS;
use web_message::Message;

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/publish/command.ts")]
pub enum PublishCommand {
	// Choose our broadcast name, or None to disable broadcasting.
	Name(Option<String>),

	// Create a new audio track.
	AudioInit {
		sample_rate: u32,
		channel_count: u32,
	},

	// Encode and publish an audio frame.
	#[ts(type = "AudioData")]
	AudioFrame(web_sys::AudioData),

	// Close the audio track.
	AudioClose,

	// Create a new video track.
	VideoInit {
		width: u32,
		height: u32,
	},

	// Encode and publish a video frame.
	#[ts(type = "VideoFrame")]
	VideoFrame(web_sys::VideoFrame),

	// Close the video track.
	VideoClose,
}
