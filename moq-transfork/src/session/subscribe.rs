use std::ops;

use crate::message;
use crate::serve::ServeError;

use crate::util::State;

use super::{Control, SessionError};

struct SubscribeState {
	// TODO
	info: Option<message::Info>,
	dropped: Vec<message::GroupDrop>,

	closed: Result<(), ServeError>,
}

impl Default for SubscribeState {
	fn default() -> Self {
		Self {
			info: None,
			dropped: Vec::default(),
			closed: Ok(()),
		}
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

	pub(super) async fn run(self, mut control: Control) -> Result<u64, SessionError> {
		control.writer.encode(&self.msg).await?;

		let info: message::Info = control.reader.decode().await?;
		self.state.lock_mut().ok_or(ServeError::Cancel)?.info = Some(info);

		while let Some(dropped) = control.reader.decode_maybe::<message::GroupDrop>().await? {
			// TODO expose to application
		}

		// TODO allow the application to update the subscription

		Ok(self.msg.id)
	}

	pub async fn info(&self) -> Result<message::Info, ServeError> {
		loop {
			{
				let state = self.state.lock();
				state.closed.clone()?;

				if let Some(info) = state.info.clone() {
					return Ok(info);
				}

				match state.modified() {
					Some(notify) => notify,
					None => return Err(ServeError::Cancel),
				}
			}
			.await;
		}
	}

	pub fn reset(&mut self, code: u32) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(ServeError::Cancel)?;
		state.closed = Err(ServeError::Closed(code as _));

		Ok(())
	}

	pub async fn closed(&self) -> Result<(), ServeError> {
		loop {
			{
				let state = self.state.lock();
				state.closed.clone()?;

				match state.modified() {
					Some(notify) => notify,
					None => return Ok(()),
				}
			}
			.await;
		}
	}
}

impl ops::Deref for Subscribe {
	type Target = message::Subscribe;

	fn deref(&self) -> &Self::Target {
		&self.msg
	}
}
