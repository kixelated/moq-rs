use ts_rs::TS;
use web_message::Message;

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/room/status.ts")]
pub enum RoomStatus {
	Init,
}
