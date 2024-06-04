use std::ops;

use crate::{message, serve::ServeError, util::State};

use super::{Control, SessionError};

struct AnnounceState {
	ok: bool,
	closed: Result<(), ServeError>,
}

impl Default for AnnounceState {
	fn default() -> Self {
		Self {
			ok: false,
			closed: Ok(()),
		}
	}
}

#[must_use = "unannounce on drop"]
pub struct Announce {
	state: State<AnnounceState>,
	msg: message::Announce,
}

impl Announce {
	pub(super) fn new(msg: message::Announce) -> Self {
		Self {
			state: Default::default(),
			msg,
		}
	}

	pub(super) fn split(&self) -> Self {
		Self {
			state: self.state.split(),
			msg: self.msg.clone(),
		}
	}

	// TODO call reader.reset and writer.reset
	pub(super) async fn run(mut self, mut control: Control) -> Result<String, SessionError> {
		// TODO handle errors
		control.writer.encode(&self.msg).await?;

		tokio::select! {
			_ = control.reader.decode::<message::AnnounceOk>() => {},
			res = self.closed() => return res.map(|_| self.msg.broadcast).map_err(Into::into),
		}

		self.state.lock_mut().ok_or(ServeError::Cancel)?.ok = true;
		control.reader.closed().await?;

		Ok(self.msg.broadcast)
	}

	pub async fn ok(&mut self) -> Result<(), SessionError> {
		loop {
			{
				let state = self.state.lock();
				if state.ok {
					return Ok(());
				}
				state.closed.clone()?;
				state.modified().ok_or(ServeError::Cancel)?
			}
			.await;
		}
	}

	// Run until we get an error
	pub async fn closed(&mut self) -> Result<(), ServeError> {
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

	pub fn reset(&mut self, code: u32) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;
		state.closed = Err(ServeError::Closed(code.into()));

		Ok(())
	}
}

impl ops::Deref for Announce {
	type Target = message::Announce;

	fn deref(&self) -> &Self::Target {
		&self.msg
	}
}
