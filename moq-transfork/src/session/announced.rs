use std::ops;

use crate::serve::ServeError;
use crate::util::State;
use crate::{message, serve};

use super::{Control, SessionError, Subscribe, Subscriber};

// There's currently no feedback from the peer, so the shared state is empty.
// If Unannounce contained an error code then we'd be talking.
struct AnnouncedState {
	closed: Result<(), ServeError>,
}

impl Default for AnnouncedState {
	fn default() -> Self {
		Self { closed: Ok(()) }
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
			res = control.reader.closed() => res,
			res = self.closed() => res.map_err(Into::into),
		}

		// TODO reset with the error code
	}

	// Helper function to subscribe to a track.
	pub fn subscribe(&mut self, track: serve::TrackWriter) -> Result<Subscribe, SessionError> {
		self.session.subscribe(&self.msg.broadcast, track)
	}

	pub fn ok(&mut self) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		if let Some(mut state) = self.state.lock_mut() {
			state.closed = Ok(());
		}

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
