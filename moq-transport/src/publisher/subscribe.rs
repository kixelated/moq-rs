use std::sync::{Arc, Mutex};

use crate::{control, data, session::SessionError, MoqError};

use super::{DatagramHeader, GroupHeader, GroupWriter, ObjectHeader, ObjectWriter, Session, TrackHeader, TrackWriter};

#[derive(Clone)]
pub struct Subscribe {
	session: Session,
	msg: control::Subscribe,

	state: Arc<Mutex<SubscribeState>>,
}

impl Subscribe {
	pub(super) fn new(session: Session, msg: control::Subscribe, state: Arc<Mutex<SubscribeState>>) -> Self {
		Self {
			state: SubscribeState::new(session.clone(), msg.id),
			session,
			msg,
		}
	}

	pub fn namespace(&self) -> &str {
		self.msg.track_namespace.as_str()
	}

	pub fn name(&self) -> &str {
		self.msg.track_name.as_str()
	}

	pub(super) fn serve(&mut self, group_id: u64, object_id: u64) -> Result<(), SessionError> {
		self.state.lock().unwrap().serve(group_id, object_id)
	}

	pub async fn serve_track(&mut self, header: TrackHeader) -> Result<TrackWriter, SessionError> {
		self.closed()?;

		let mut stream = self.session.open_uni().await?;

		let header = data::TrackHeader {
			subscribe_id: self.msg.id,
			track_alias: self.msg.track_alias,
			send_order: header.send_order,
		};
		header.encode(&mut stream).await?;

		let track = TrackWriter::new(self.clone(), stream);
		Ok(track)
	}

	pub async fn serve_group(&mut self, header: GroupHeader) -> Result<GroupWriter, SessionError> {
		self.closed()?;

		let mut stream = self.session.open_uni().await?;

		let header = data::GroupHeader {
			subscribe_id: self.msg.id,
			track_alias: self.msg.track_alias,
			group_id: header.group_id,
			send_order: header.send_order,
		};
		header.encode(&mut stream).await?;

		let group = GroupWriter::new(self.clone(), stream, header.group_id);

		Ok(group)
	}

	pub async fn serve_object(&mut self, header: ObjectHeader) -> Result<ObjectWriter, SessionError> {
		self.state.lock().unwrap().serve(header.group_id, header.object_id)?;

		let mut stream = self.session.open_uni().await?;

		let header = data::GroupHeader {
			subscribe_id: self.msg.id,
			track_alias: self.msg.track_alias,
			group_id: header.group_id,
			send_order: header.send_order,
		};
		header.encode(&mut stream).await?;

		let object = ObjectWriter::new(stream);

		Ok(object)
	}

	pub fn serve_datagram(&mut self, header: DatagramHeader, payload: &[u8]) -> Result<(), SessionError> {
		self.state.lock().unwrap().serve(header.group_id, header.object_id)?;

		let header = data::DatagramHeader {
			subscribe_id: self.msg.id,
			track_alias: self.msg.track_alias,
			group_id: header.group_id,
			object_id: header.object_id,
			send_order: header.send_order,
		};

		unimplemented!("TODO encode datagram");

		// self.session.webtransport().send_datagram(&header, &payload)?;
	}

	pub fn close(&mut self, code: u64) -> Result<(), SessionError> {
		self.state.lock().unwrap().close(SessionError::Done(code))
	}

	pub fn closed(&self) -> Result<(), SessionError> {
		self.state.lock().unwrap().closed
	}

	fn ok(&mut self, response: SubscribeResponse) -> Result<(), SessionError> {
		self.state.lock().unwrap().ok()?;

		self.session.send_message(control::SubscribeOk {
			id: self.msg.id,
			latest: response.latest,
			expires: response.expires,
		})?;

		Ok(())
	}

	fn reject(&mut self, code: u64) -> Result<(), SessionError> {
		self.state.lock().unwrap().close(SessionError::Reject(code))
	}
}

pub(super) struct SubscribeState {
	session: Session,
	id: u64,

	ok: bool,
	max: Option<(u64, u64)>,
	closed: Result<(), SessionError>,
}

impl SubscribeState {
	pub(super) fn new(session: Session, id: u64) -> Arc<Mutex<Self>> {
		Arc::new(Mutex::new(Self {
			session,
			id,
			ok: false,
			max: None,
			closed: Ok(()),
		}))
	}

	pub(super) fn close(&mut self, err: SessionError) -> Result<(), SessionError> {
		self.closed?;
		self.closed = Err(err);

		if self.ok {
			self.session.send_message(control::SubscribeDone {
				id: self.id,
				last: self.max,
				code: err.code().into(),
				reason: err.to_string(),
			})?;
		} else {
			self.session.send_message(control::SubscribeError {
				id: self.id,
				alias: 0,
				code: err.code().into(),
				reason: err.to_string(),
			})?;
		}

		self.session.remove_subscribe(self.id);

		Ok(())
	}

	fn ok(&mut self) -> Result<(), SessionError> {
		self.closed?;
		self.ok = true;
		Ok(())
	}

	fn serve(&mut self, group_id: u64, object_id: u64) -> Result<(), SessionError> {
		self.closed?;

		if let Some((max_group, max_object)) = self.max {
			if group_id >= max_group && object_id >= max_object {
				self.max = Some((group_id, object_id));
			}
		}

		Ok(())
	}
}

impl Drop for SubscribeState {
	fn drop(&mut self) {
		self.close(SessionError::Dropped).ok();
	}
}

pub struct SubscribeRequest {
	subscribe: Subscribe,
}

pub struct SubscribeResponse {
	// The maximum group/object seen thus far
	latest: Option<(u64, u64)>,

	// The amount of seconds before we'll terminate the subscription
	expires: Option<u64>,
}

impl SubscribeRequest {
	pub(crate) fn new(subscribe: Subscribe) -> Self {
		Self { subscribe }
	}

	pub fn namespace(&self) -> &str {
		self.subscribe.namespace()
	}

	pub fn name(&self) -> &str {
		self.subscribe.name()
	}

	// Send a SUBSCRIBE_OK
	pub fn accept(mut self, response: SubscribeResponse) -> Result<Subscribe, SessionError> {
		self.subscribe.ok(response)?;
		Ok(self.subscribe)
	}

	// Send a SUBSCRIBE_ERROR
	pub fn reject(mut self, code: u64) -> Result<(), SessionError> {
		self.subscribe.reject(code)
	}
}
