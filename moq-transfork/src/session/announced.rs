use std::ops;

use crate::message;
use crate::serve::ServeError;
use crate::util::State;

use super::{Control, SessionError, Subscriber};

// There's currently no feedback from the peer, so the shared state is empty.
// If Unannounce contained an error code then we'd be talking.
struct AnnouncedState {
	ok: bool,
	closed: Result<(), ServeError>,
}

impl Default for AnnouncedState {
	fn default() -> Self {
		Self {
			ok: false,
			closed: Ok(()),
		}
	}
}

#[must_use = "unannounce on drop"]
#[derive(Clone)]
pub struct Announced {
	session: Subscriber,
	msg: message::Announce,
	state: State<AnnouncedState>,
}

impl Announced {
	pub(super) fn new(session: Subscriber, msg: message::Announce) -> Self {
		Self {
			session,
			msg,
			state: Default::default(),
		}
	}

	pub(super) fn split(&self) -> Self {
		Self {
			session: self.session.clone(),
			msg: self.msg.clone(),
			state: self.state.split(),
		}
	}

	pub(super) async fn run(self, mut control: Control) -> Result<(), SessionError> {
		// Wait until either the reader or the session is closed.
		tokio::select! {
			res = control.reader.closed() => return res,
			res = self.acked() => res?,
		};

		// Send the OK message.
		let msg = message::AnnounceOk {};
		control.writer.encode(&msg).await?;

		// Wait until the reader is closed.
		tokio::select! {
			res = control.reader.closed() => res,
			res = self.closed() => res.map_err(Into::into),
		}

		// TODO reset with the error code
	}

	/// Reply OK to the announcement.
	pub fn ack(&mut self) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		self.state.lock_mut().ok_or(ServeError::Cancel)?.ok = true;

		Ok(())
	}

	// Wait until we've acknowledged the announcement.
	async fn acked(&self) -> Result<(), ServeError> {
		loop {
			{
				let state = self.state.lock();
				state.closed.clone()?;

				if state.ok {
					return Ok(());
				}

				state.modified().ok_or(ServeError::Cancel)?
			}
			.await;
		}
	}

	/// Wait for the announcement to be closed.
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

	pub fn close(&mut self, code: u32) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		if let Some(mut state) = self.state.lock_mut() {
			state.closed = Err(ServeError::Closed(code as _));
		}

		Ok(())
	}
}

impl ops::Deref for Announced {
	type Target = message::Announce;

	fn deref(&self) -> &Self::Target {
		&self.msg
	}
}
