use std::sync::{Arc, Mutex};

use crate::{
	data,
	message::{self, SubscribePair},
	serve::{self, ServeError},
	util::Watch,
};

use super::{SessionError, Subscriber};

pub struct Subscribe {
	session: Subscriber,
	id: u64,
	track: serve::TrackSubscriber,
	state: Watch<State>,
}

impl Subscribe {
	pub(super) fn new(session: Subscriber, msg: message::Subscribe) -> (SubscribeRecv, Subscribe) {
		let state = Watch::new(State::default());

		let (publisher, subscriber) = serve::Track {
			namespace: msg.track_namespace,
			name: msg.track_name.clone(),
		}
		.produce();

		// TODO apply start/end range

		let subscriber = Subscribe {
			session,
			id: msg.id,
			track: subscriber,
			state: state.clone(),
		};

		let publisher = SubscribeRecv::new(state, publisher);

		(publisher, subscriber)
	}

	// Waits until an OK message is received.
	pub async fn ok(&self) -> Result<(), ServeError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if state.ok.is_some() {
					return Ok(());
				}
				state.changed()
			};

			tokio::select! {
				_ = notify => {},
				err = self.track.closed() => return err,
			};
		}
	}

	// Returns the maximum known group/object sequences.
	pub fn max(&self) -> Option<(u64, u64)> {
		let ok = self.state.lock().ok.as_ref().and_then(|ok| ok.latest);
		let cache = self.track.latest();

		// Return the max of both the OK message and the cache.
		match ok {
			Some(ok) => match cache {
				Some(cache) => Some(cache.max(ok)),
				None => Some(ok),
			},
			None => cache,
		}
	}

	pub fn track(&self) -> serve::TrackSubscriber {
		self.track.clone()
	}
}

impl Drop for Subscribe {
	fn drop(&mut self) {
		let msg = message::Unsubscribe { id: self.id };
		self.session.send_message(msg).ok();

		self.session.drop_subscribe(self.id);
	}
}

#[derive(Clone)]
pub(super) struct SubscribeRecv {
	publisher: Arc<Mutex<serve::TrackPublisher>>,
	state: Watch<State>,
}

impl SubscribeRecv {
	fn new(state: Watch<State>, publisher: serve::TrackPublisher) -> Self {
		Self {
			publisher: Arc::new(Mutex::new(publisher)),
			state,
		}
	}

	pub fn recv_ok(&mut self, msg: message::SubscribeOk) -> Result<(), ServeError> {
		let mut state = self.state.lock_mut();
		state.ok = Some(msg);
		Ok(())
	}

	pub fn recv_error(&mut self, code: u64) -> Result<(), ServeError> {
		self.publisher.lock().unwrap().close(ServeError::Closed(code))?;
		Ok(())
	}

	pub fn recv_done(&mut self, code: u64) -> Result<(), ServeError> {
		self.publisher.lock().unwrap().close(ServeError::Closed(code))?;
		Ok(())
	}

	pub async fn recv_stream(
		&mut self,
		header: data::Header,
		stream: webtransport_quinn::RecvStream,
	) -> Result<(), SessionError> {
		match header {
			data::Header::Track(track) => self.recv_track(track, stream).await,
			data::Header::Group(group) => self.recv_group(group, stream).await,
			data::Header::Object(object) => self.recv_object(object, stream).await,
		}
	}

	async fn recv_track(
		&mut self,
		header: data::TrackHeader,
		mut stream: webtransport_quinn::RecvStream,
	) -> Result<(), SessionError> {
		log::trace!("received track: {:?}", header);

		let mut track = self.publisher.lock().unwrap().create_stream(header.send_order)?;

		while let Some(chunk) = data::TrackObject::decode(&mut stream).await? {
			let mut remain = chunk.size;

			let mut chunks = vec![];
			while remain > 0 {
				let chunk = stream.read_chunk(remain, true).await?.ok_or(SessionError::WrongSize)?;
				log::trace!("received track payload: {:?}", chunk.bytes.len());
				remain -= chunk.bytes.len();
				chunks.push(chunk.bytes);
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
		mut stream: webtransport_quinn::RecvStream,
	) -> Result<(), SessionError> {
		log::trace!("received group: {:?}", header);

		let mut group = self.publisher.lock().unwrap().create_group(serve::Group {
			id: header.group_id,
			send_order: header.send_order,
		})?;

		while let Some(object) = data::GroupObject::decode(&mut stream).await? {
			log::trace!("received group object: {:?}", object);
			let mut remain = object.size;
			let mut object = group.create_object(object.size)?;

			while remain > 0 {
				let data = stream.read_chunk(remain, true).await?.ok_or(SessionError::WrongSize)?;
				log::trace!("received group payload: {:?}", data.bytes.len());
				remain -= data.bytes.len();
				object.write(data.bytes)?;
			}
		}

		Ok(())
	}

	async fn recv_object(
		&mut self,
		header: data::ObjectHeader,
		mut stream: webtransport_quinn::RecvStream,
	) -> Result<(), SessionError> {
		log::trace!("received object: {:?}", header);

		// TODO avoid buffering the entire object to learn the size.
		let mut chunks = vec![];
		while let Some(data) = stream.read_chunk(usize::MAX, true).await? {
			log::trace!("received object payload: {:?}", data.bytes.len());
			chunks.push(data.bytes);
		}

		let mut object = self.publisher.lock().unwrap().create_object(serve::ObjectHeader {
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

		self.publisher.lock().unwrap().write_datagram(serve::Datagram {
			group_id: datagram.group_id,
			object_id: datagram.object_id,
			payload: datagram.payload,
			send_order: datagram.send_order,
		})?;

		Ok(())
	}
}

#[derive(Default)]
struct State {
	ok: Option<message::SubscribeOk>,
}

#[derive(Default)]
pub struct SubscribeOptions {
	pub start: SubscribePair,
	pub end: SubscribePair,
}
