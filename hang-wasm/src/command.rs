use ts_rs::TS;
use web_message::Message;

#[derive(Debug, Message, TS)]
#[msg(tag = "command")]
#[ts(tag = "command", export, export_to = "../src/command.ts")]
pub enum Command {
	Connect {
		#[ts(type = "URL")]
		url: web_sys::Url,
	},
	Disconnect,

	Play,
	Stop,
	Seek {
		position: f64,
	},
	Volume {
		volume: f64,
	},
	Canvas {
		#[msg(transferable)]
		#[ts(type = "OffscreenCanvas")]
		canvas: web_sys::OffscreenCanvas,
	},
	Latency {
		latency: u64,
	},
}
