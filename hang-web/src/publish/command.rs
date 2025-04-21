use ts_rs::TS;
use url::Url;
use web_message::Message;

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/publish/command.ts")]
pub enum PublishCommand {
	// Publish a broadcast with the given URL.
	// ex. https://relay.quic.video/demo/bbb
	Connect(Option<Url>),

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
