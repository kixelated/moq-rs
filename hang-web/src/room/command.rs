use ts_rs::TS;
use url::Url;
use web_message::Message;

use crate::{PublishCommand, WatchCommand};

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/room/command.ts")]
pub enum RoomCommand {
	Connect { url: Option<Url> },
	Publish(PublishCommand),
	Watch(WatchCommand),
}
