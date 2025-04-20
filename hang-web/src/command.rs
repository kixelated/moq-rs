use ts_rs::TS;
use web_message::Message;

use crate::{PublishCommand, WatchCommand};

#[derive(Message, Debug, TS)]
#[ts(export, export_to = "../src/command.ts")]
pub enum Command {
	// Responsible for rendering a broadcast.
	Watch(WatchCommand),

	// Responsible for publishing a broadcast.
	Publish(PublishCommand),
}
