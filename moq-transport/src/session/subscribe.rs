use std::ops;

use crate::{
	data, message,
	serve::{self, ServeError, TrackWriter, TrackWriterMode},
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
			start: Default::default(),
			end: Default::default(),
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
			writer: Some(track.into()),
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
	writer: Option<TrackWriterMode>,
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
		if let Some(writer) = self.writer.take() {
			writer.close(err.clone())?;
		}

		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(ServeError::Cancel)?;
		state.closed = Err(err);

		Ok(())
	}

	pub fn track(&mut self, header: data::TrackHeader) -> Result<serve::StreamWriter, ServeError> {
		let writer = self.writer.take().ok_or(ServeError::Done)?;

		let stream = match writer {
			TrackWriterMode::Track(init) => init.stream(header.send_order)?,
			_ => return Err(ServeError::Mode),
		};

		self.writer = Some(stream.clone().into());

		Ok(stream)
	}

	pub fn group(&mut self, header: data::GroupHeader) -> Result<serve::GroupWriter, ServeError> {
		let writer = self.writer.take().ok_or(ServeError::Done)?;

		let mut groups = match writer {
			TrackWriterMode::Track(init) => init.groups()?,
			TrackWriterMode::Groups(groups) => groups,
			_ => return Err(ServeError::Mode),
		};

		let writer = groups.create(serve::Group {
			group_id: header.group_id,
			priority: header.send_order,
		})?;

		self.writer = Some(groups.into());

		Ok(writer)
	}

	pub fn object(&mut self, header: data::ObjectHeader) -> Result<serve::ObjectWriter, ServeError> {
		let writer = self.writer.take().ok_or(ServeError::Done)?;

		let mut objects = match writer {
			TrackWriterMode::Track(init) => init.objects()?,
			TrackWriterMode::Objects(objects) => objects,
			_ => return Err(ServeError::Mode),
		};

		let writer = objects.create(serve::Object {
			group_id: header.group_id,
			object_id: header.object_id,
			priority: header.send_order,
		})?;

		self.writer = Some(objects.into());

		Ok(writer)
	}

	pub fn datagram(&mut self, datagram: data::Datagram) -> Result<(), ServeError> {
		let writer = self.writer.take().ok_or(ServeError::Done)?;

		let mut datagrams = match writer {
			TrackWriterMode::Track(init) => init.datagrams()?,
			TrackWriterMode::Datagrams(datagrams) => datagrams,
			_ => return Err(ServeError::Mode),
		};

		datagrams.write(serve::Datagram {
			group_id: datagram.group_id,
			object_id: datagram.object_id,
			priority: datagram.send_order,
			payload: datagram.payload,
		})?;

		Ok(())
	}
}
