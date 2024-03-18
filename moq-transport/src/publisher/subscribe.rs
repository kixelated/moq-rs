use std::sync::{Arc, Mutex, Weak};

use crate::{
	control, data,
	error::{SubscribeError, WriteError},
};

use super::{Datagram, GroupHeader, GroupStream, ObjectHeader, ObjectStream, Publisher, TrackHeader, TrackStream};

#[derive(Clone)]
pub struct Subscribe {
	session: Publisher,
	msg: control::Subscribe,
	state: Arc<Mutex<SubscribeState>>,
}

impl Subscribe {
	pub(super) fn new(session: Publisher, msg: control::Subscribe) -> Self {
		let state = SubscribeState::new(session.clone(), msg.id);
		Self { session, msg, state }
	}

	pub fn namespace(&self) -> &str {
		self.msg.track_namespace.as_str()
	}

	pub fn name(&self) -> &str {
		self.msg.track_name.as_str()
	}

	pub async fn serve_track(&mut self, header: TrackHeader) -> Result<TrackStream, WriteError> {
		self.closed()?;

		let mut stream = self.session.webtransport().open_uni().await?;

		let header = data::TrackHeader {
			subscribe_id: self.msg.id,
			track_alias: self.msg.track_alias,
			send_order: header.send_order,
		};
		header.encode(&mut stream).await?;

		let track = TrackStream::new(self.clone(), stream);
		Ok(track)
	}

	pub async fn serve_group(&mut self, header: GroupHeader) -> Result<GroupStream, WriteError> {
		self.closed()?;

		let mut stream = self.session.webtransport().open_uni().await?;

		let header = data::GroupHeader {
			subscribe_id: self.msg.id,
			track_alias: self.msg.track_alias,
			group_id: header.group_id,
			send_order: header.send_order,
		};
		header.encode(&mut stream).await?;

		let group = GroupStream::new(self.clone(), stream, header.group_id);

		Ok(group)
	}

	pub async fn serve_object(&mut self, header: ObjectHeader) -> Result<ObjectStream, WriteError> {
		let mut stream = self.session.webtransport().open_uni().await?;

		let header = data::ObjectHeader {
			subscribe_id: self.msg.id,
			track_alias: self.msg.track_alias,
			group_id: header.group_id,
			object_id: header.object_id,
			send_order: header.send_order,
		};
		header.encode(&mut stream).await?;

		let object = ObjectStream::new(stream);

		// TODO call this on payload write instead
		self.state
			.lock()
			.unwrap()
			.update_max(header.group_id, header.object_id)?;

		Ok(object)
	}

	pub fn serve_datagram(&mut self, datagram: Datagram) -> Result<(), SubscribeError> {
		let _header = data::Datagram {
			subscribe_id: self.msg.id,
			track_alias: self.msg.track_alias,
			group_id: datagram.group_id,
			object_id: datagram.object_id,
			send_order: datagram.send_order,
			payload: datagram.payload,
		};

		unimplemented!("TODO encode datagram");

		/*
		self.session.webtransport().send_datagram(&header, &payload)?;

		self.state
			.lock()
			.unwrap()
			.update_max(header.group_id, header.object_id)?;
		*/
	}

	pub(super) fn update_max(&mut self, group_id: u64, object_id: u64) -> Result<(), SubscribeError> {
		self.state.lock().unwrap().update_max(group_id, object_id)
	}

	pub fn close(&mut self, err: SubscribeError) -> Result<(), SubscribeError> {
		self.state.lock().unwrap().close(err)
	}

	pub fn closed(&self) -> Result<(), SubscribeError> {
		self.state.lock().unwrap().closed()
	}

	fn ok(&mut self, response: SubscribeResponse) -> Result<(), SubscribeError> {
		self.state.lock().unwrap().ok()?;

		self.session.send_message(control::SubscribeOk {
			id: self.msg.id,
			latest: response.latest,
			expires: response.expires,
		})?;

		Ok(())
	}

	fn reject(&mut self, err: SubscribeError) -> Result<(), SubscribeError> {
		self.state.lock().unwrap().close(err)
	}

	pub(super) fn downgrade(&self) -> SubscribeWeak {
		SubscribeWeak {
			state: Arc::downgrade(&self.state),
			session: self.session.clone(),
			msg: self.msg.clone(),
		}
	}
}

#[derive(Clone)]
pub(super) struct SubscribeWeak {
	state: Weak<Mutex<SubscribeState>>,
	session: Publisher,
	msg: control::Subscribe,
}

impl SubscribeWeak {
	pub fn upgrade(&self) -> Option<Subscribe> {
		Some(Subscribe {
			state: self.state.upgrade()?,
			session: self.session.clone(),
			msg: self.msg.clone(),
		})
	}
}

struct SubscribeState {
	session: Publisher,
	id: u64,

	ok: bool,
	max: Option<(u64, u64)>,
	closed: Result<(), SubscribeError>,
}

impl SubscribeState {
	fn new(session: Publisher, id: u64) -> Arc<Mutex<Self>> {
		Arc::new(Mutex::new(Self {
			session,
			id,
			ok: false,
			max: None,
			closed: Ok(()),
		}))
	}

	fn close(&mut self, err: SubscribeError) -> Result<(), SubscribeError> {
		self.closed()?;
		self.closed = Err(err.clone());

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

		Ok(())
	}

	fn closed(&self) -> Result<(), SubscribeError> {
		self.closed.clone()
	}

	fn ok(&mut self) -> Result<(), SubscribeError> {
		self.closed()?;
		self.ok = true;
		Ok(())
	}

	fn update_max(&mut self, group_id: u64, object_id: u64) -> Result<(), SubscribeError> {
		self.closed()?;

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
		self.close(SubscribeError::Done(0)).ok();
		self.session.drop_subscribe(self.id);
	}
}

pub struct SubscribePending {
	subscribe: Subscribe,
}

pub struct SubscribeResponse {
	// The maximum group/object seen thus far
	pub latest: Option<(u64, u64)>,

	// The amount of seconds before we'll terminate the subscription
	pub expires: Option<u64>,
}

impl SubscribePending {
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
	pub fn accept(mut self, response: SubscribeResponse) -> Result<Subscribe, SubscribeError> {
		self.subscribe.ok(response)?;
		Ok(self.subscribe)
	}

	// Send a SUBSCRIBE_ERROR
	pub fn reject(mut self, code: u64) -> Result<(), SubscribeError> {
		self.subscribe.reject(SubscribeError::Error(code))
	}
}
