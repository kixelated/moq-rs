use ts_rs::TS;
use web_message::Message;

use crate::{PublishCommand, WatchCommand};

#[derive(Message, Debug, TS)]
#[ts(export, export_to = "../src/message.ts")]
pub enum Command {
	// Responsible for rendering a broadcast.
	Watch(WatchCommand),

	// Responsible for publishing a broadcast.
	Publish(PublishCommand),
}

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/message.ts")]
pub enum Event {
	Init,
	Connection(ConnectionStatus),
}

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/message.ts")]
pub enum ConnectionStatus {
	Disconnected,
	Connecting,
	Connected,
	Live,
	Offline,
	Error(String),
}

impl From<ConnectionStatus> for Event {
	fn from(status: ConnectionStatus) -> Self {
		Event::Connection(status)
	}
}
