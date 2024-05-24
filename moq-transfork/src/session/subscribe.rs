use std::ops;

use crate::{
	data,
	message::{self, SubscribeLocation, SubscribePair},
	serve::{self, ServeError, TrackWriter},
};

use crate::watch::State;

use super::Subscriber;

#[derive(Debug, Clone)]
pub struct SubscribeInfo {
	pub namespace: String,
	pub name: String,
}

struct SubscribeState {
	ok: bool,
	closed: Result<(), ServeError>,
}

impl Default for SubscribeState {
	fn default() -> Self {
		Self {
			ok: Default::default(),
			closed: Ok(()),
		}
	}
}

// Held by the application
#[must_use = "unsubscribe on drop"]
pub struct Subscribe {
	state: State<SubscribeState>,
	subscriber: Subscriber,
	id: u64,

	pub info: SubscribeInfo,
}

impl Subscribe {
	pub(super) fn new(mut subscriber: Subscriber, id: u64, track: TrackWriter) -> (Subscribe, SubscribeRecv) {
		subscriber.send_message(message::Subscribe {
			id,
			track_alias: id,
			track_namespace: track.namespace.clone(),
			track_name: track.name.clone(),
			// TODO add these to the publisher.
			start: SubscribePair {
				group: SubscribeLocation::Latest(0),
				object: SubscribeLocation::Absolute(0),
			},
			end: SubscribePair {
				group: SubscribeLocation::None,
				object: SubscribeLocation::None,
			},
			params: Default::default(),
		});

		let info = SubscribeInfo {
			namespace: track.namespace.clone(),
			name: track.name.clone(),
		};

		let (send, recv) = State::default().split();

		let send = Subscribe {
			state: send,
			subscriber,
			id,
			info,
		};

		let recv = SubscribeRecv {
			state: recv,
			writer: track,
		};

		(send, recv)
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

impl Drop for Subscribe {
	fn drop(&mut self) {
		self.subscriber.send_message(message::Unsubscribe { id: self.id });
	}
}

impl ops::Deref for Subscribe {
	type Target = SubscribeInfo;

	fn deref(&self) -> &SubscribeInfo {
		&self.info
	}
}

pub(super) struct SubscribeRecv {
	state: State<SubscribeState>,
	writer: TrackWriter,
}

impl SubscribeRecv {
	pub fn ok(&mut self) -> Result<(), ServeError> {
		let state = self.state.lock();
		if state.ok {
			return Err(ServeError::Duplicate);
		}

		if let Some(mut state) = state.into_mut() {
			state.ok = true;
		}

		Ok(())
	}

	pub fn error(mut self, err: ServeError) -> Result<(), ServeError> {
		self.writer.close(err.clone())?;

		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(ServeError::Cancel)?;
		state.closed = Err(err);

		Ok(())
	}

	pub fn recv_group(&mut self, header: data::GroupHeader) -> Result<serve::GroupWriter, ServeError> {
		let group = self.writer.create(serve::Group {
			group_id: header.group_id,
			priority: header.send_order,
		})?;

		Ok(group)
	}
}
