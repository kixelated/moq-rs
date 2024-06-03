use std::ops;

use crate::message;
use crate::serve::{self, ServeError, TrackWriter};

use crate::util::State;

use super::{Control, Reader, SessionError, Subscriber, Writer};

struct SubscribeState {
	// TODO
	// info: Option<message::Info>,
	// dropped: Vec<message::GroupDrop>,
	closed: Result<(), ServeError>,
}

impl Default for SubscribeState {
	fn default() -> Self {
		Self { closed: Ok(()) }
	}
}

#[must_use = "unsubscribe on drop"]
pub struct Subscribe {
	msg: message::Subscribe,
	state: State<SubscribeState>,
}

impl Subscribe {
	pub(super) fn new(msg: message::Subscribe) -> Self {
		Self {
			msg,
			state: Default::default(),
		}
	}

	pub(super) fn split(&self) -> Self {
		Self {
			msg: self.msg.clone(),
			state: self.state.split(),
		}
	}

	pub(super) async fn run(mut self, mut control: Control) -> Result<u64, SessionError> {
		control.writer.encode(&message::StreamBi::Subscribe).await?;
		control.writer.encode(&self.msg).await?;

		let info: message::Info = control.reader.decode().await?;
		// TODO expose to application

		while let Some(dropped) = control.reader.decode_maybe::<message::GroupDrop>().await {
			// TODO expose to application
		}

		// TODO allow the application to update the subscription

		Ok(self.msg.id)
	}
}
