use crate::{
	data, message,
	serve::{self, ServeError, TrackWriter, TrackWriterMode},
};

use super::{SessionResult, Subscriber};

// Held by the application
pub(super) struct Subscribe<S: webtransport_generic::Session> {
	session: Subscriber<S>,
	id: u64,
	writer: Option<TrackWriterMode>,
}

impl<S: webtransport_generic::Session> Subscribe<S> {
	pub fn new(mut session: Subscriber<S>, id: u64, track: TrackWriter) -> SessionResult<Self> {
		session.send_message(message::Subscribe {
			id,
			track_alias: id,
			track_namespace: track.namespace.clone(),
			track_name: track.name.clone(),
			// TODO add these to the publisher.
			start: Default::default(),
			end: Default::default(),
			params: Default::default(),
		})?;

		Ok(Self {
			session,
			id,
			writer: Some(track.into()),
		})
	}

	pub fn recv_ok(&mut self, _msg: message::SubscribeOk) -> Result<(), ServeError> {
		// TODO
		Ok(())
	}

	pub fn recv_error(&mut self, code: u64) -> Result<(), ServeError> {
		self.close(ServeError::Closed(code))
	}

	pub fn recv_done(&mut self, code: u64) -> Result<(), ServeError> {
		self.close(ServeError::Closed(code))
	}

	fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.writer.as_mut().unwrap().close(err)
	}

	pub fn recv_track(&mut self, header: data::TrackHeader) -> Result<serve::StreamWriter, ServeError> {
		let stream = match self.writer.take().unwrap() {
			TrackWriterMode::Track(init) => init.stream(header.send_order),
			_ => return Err(ServeError::Mode),
		};

		self.writer = Some(stream.clone().into());

		Ok(stream)
	}

	pub fn recv_group(&mut self, header: data::GroupHeader) -> Result<serve::GroupWriter, ServeError> {
		let mut groups = match self.writer.take().unwrap() {
			TrackWriterMode::Track(init) => init.groups(),
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

	pub fn recv_object(&mut self, header: data::ObjectHeader) -> Result<serve::ObjectWriter, ServeError> {
		let mut objects = match self.writer.take().unwrap() {
			TrackWriterMode::Track(init) => init.objects(),
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

	pub fn recv_datagram(&mut self, datagram: data::Datagram) -> Result<(), ServeError> {
		let mut datagrams = match self.writer.take().unwrap() {
			TrackWriterMode::Track(init) => init.datagrams(),
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

impl<S: webtransport_generic::Session> Drop for Subscribe<S> {
	fn drop(&mut self) {
		let msg = message::Unsubscribe { id: self.id };
		self.session.send_message(msg).ok();
	}
}
