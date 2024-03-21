use std::sync::{Arc, Mutex};

use crate::{
	coding::Reader,
	data, message,
	serve::{self, ServeError},
};

use super::{SessionError, Subscriber};

#[derive(Clone)]
pub struct Subscribe<S: webtransport_generic::Session> {
	session: Subscriber<S>,
	id: u64,
	track: Arc<Mutex<serve::TrackPublisher>>,
}

impl<S: webtransport_generic::Session> Subscribe<S> {
	pub(super) fn new(session: Subscriber<S>, id: u64, track: serve::TrackPublisher) -> Self {
		Self {
			session,
			id,
			track: Arc::new(Mutex::new(track)),
		}
	}

	pub fn recv_ok(&mut self, _msg: message::SubscribeOk) -> Result<(), ServeError> {
		// TODO
		Ok(())
	}

	pub fn recv_error(&mut self, code: u64) -> Result<(), ServeError> {
		self.track.lock().unwrap().close(ServeError::Closed(code))?;
		Ok(())
	}

	pub fn recv_done(&mut self, code: u64) -> Result<(), ServeError> {
		self.track.lock().unwrap().close(ServeError::Closed(code))?;
		Ok(())
	}

	pub async fn recv_stream(
		&mut self,
		header: data::Header,
		reader: Reader<S::RecvStream>,
	) -> Result<(), SessionError> {
		match header {
			data::Header::Track(track) => self.recv_track(track, reader).await,
			data::Header::Group(group) => self.recv_group(group, reader).await,
			data::Header::Object(object) => self.recv_object(object, reader).await,
		}
	}

	async fn recv_track(
		&mut self,
		header: data::TrackHeader,
		mut reader: Reader<S::RecvStream>,
	) -> Result<(), SessionError> {
		log::trace!("received track: {:?}", header);

		let mut track = self.track.lock().unwrap().create_stream(header.send_order)?;

		while !reader.done().await? {
			let chunk: data::TrackObject = reader.decode().await?;

			let mut remain = chunk.size;

			let mut chunks = vec![];
			while remain > 0 {
				let chunk = reader.read(remain).await?.ok_or(SessionError::WrongSize)?;
				log::trace!("received track payload: {:?}", chunk.len());
				remain -= chunk.len();
				chunks.push(chunk);
			}

			let object = serve::StreamObject {
				object_id: chunk.object_id,
				group_id: chunk.group_id,
				payload: bytes::Bytes::from(chunks.concat()),
			};
			log::trace!("received track object: {:?}", track);

			track.write_object(object)?;
		}

		Ok(())
	}

	async fn recv_group(
		&mut self,
		header: data::GroupHeader,
		mut reader: Reader<S::RecvStream>,
	) -> Result<(), SessionError> {
		log::trace!("received group: {:?}", header);

		let mut group = self.track.lock().unwrap().create_group(serve::Group {
			id: header.group_id,
			send_order: header.send_order,
		})?;

		while !reader.done().await? {
			let object: data::GroupObject = reader.decode().await?;

			log::trace!("received group object: {:?}", object);
			let mut remain = object.size;
			let mut object = group.create_object(object.size)?;

			while remain > 0 {
				let data = reader.read(remain).await?.ok_or(SessionError::WrongSize)?;
				log::trace!("received group payload: {:?}", data.len());
				remain -= data.len();
				object.write(data)?;
			}
		}

		Ok(())
	}

	async fn recv_object(
		&mut self,
		header: data::ObjectHeader,
		mut reader: Reader<S::RecvStream>,
	) -> Result<(), SessionError> {
		log::trace!("received object: {:?}", header);

		// TODO avoid buffering the entire object to learn the size.
		let mut chunks = vec![];
		while let Some(data) = reader.read(usize::MAX).await? {
			log::trace!("received object payload: {:?}", data.len());
			chunks.push(data);
		}

		let mut object = self.track.lock().unwrap().create_object(serve::ObjectHeader {
			group_id: header.group_id,
			object_id: header.object_id,
			send_order: header.send_order,
			size: chunks.iter().map(|c| c.len()).sum(),
		})?;

		log::trace!("received object: {:?}", object);

		for chunk in chunks {
			object.write(chunk)?;
		}

		Ok(())
	}

	pub fn recv_datagram(&self, datagram: data::Datagram) -> Result<(), SessionError> {
		log::trace!("received datagram: {:?}", datagram);

		self.track.lock().unwrap().write_datagram(serve::Datagram {
			group_id: datagram.group_id,
			object_id: datagram.object_id,
			payload: datagram.payload,
			send_order: datagram.send_order,
		})?;

		Ok(())
	}
}

impl<S: webtransport_generic::Session> Drop for Subscribe<S> {
	fn drop(&mut self) {
		let msg = message::Unsubscribe { id: self.id };
		self.session.send_message(msg).ok();
		self.session.drop_subscribe(self.id);
	}
}
