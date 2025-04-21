use ts_rs::TS;
use web_message::Message;

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/watch/worklet/message.ts")]
pub enum WorkletCommand {
	#[ts(type = "AudioData")]
	Frame(web_sys::AudioData),
}
