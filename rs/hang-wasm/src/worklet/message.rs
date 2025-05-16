use ts_rs::TS;
use web_message::Message;

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/worklet/message.ts")]
pub enum WorkletCommand {
	/// A frame of audio data split into channels.
	// I tried to transfer an AudioData object to the Worklet but it silently failed.
	// So instead we need to copy/allocate the data into a Vec<Float32Array> and transfer that.
	Frame {
		#[ts(type = "Float32Array[]")]
		channels: Vec<js_sys::Float32Array>,
		timestamp: u64,
	},
}
