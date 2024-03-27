use futures::stream::FuturesUnordered;
use futures::StreamExt;

use crate::coding::{Encode, Writer};
use crate::serve::{ServeError, TrackReaderMode};
use crate::util::State;
use crate::{data, message, serve, Publisher};

use super::SessionError;

#[derive(Debug)]
struct SubscribedState {
	ok: bool,
	max: Option<(u64, u64)>,
	closed: Result<(), ServeError>, // The error we sent
}

impl SubscribedState {
	fn update_max(&mut self, group_id: u64, object_id: u64) -> Result<(), ServeError> {
		if let Some((max_group, max_object)) = self.max {
			if group_id >= max_group && object_id >= max_object {
				self.max = Some((group_id, object_id));
			}
		}

		Ok(())
	}
}

impl Default for SubscribedState {
	fn default() -> Self {
		Self {
			ok: false,
			max: None,
			closed: Ok(()),
		}
	}
}

pub struct Subscribed<S: webtransport_generic::Session> {
	publisher: Publisher<S>,
	state: State<SubscribedState>,
	msg: message::Subscribe,
}

impl<S: webtransport_generic::Session> Subscribed<S> {
	pub(super) fn new(publisher: Publisher<S>, msg: message::Subscribe) -> (Self, SubscribedRecv) {
		let (send, recv) = State::default();

		let send = Self {
			publisher,
			state: send,
			msg,
		};

		// Prevents updates after being closed
		// TODO actively abort on close
		let recv = SubscribedRecv { _state: recv };

		(send, recv)
	}

	pub fn namespace(&self) -> &str {
		self.msg.track_namespace.as_str()
	}

	pub fn name(&self) -> &str {
		self.msg.track_name.as_str()
	}

	pub async fn serve(mut self, track: serve::TrackReader) -> Result<(), SessionError> {
		if let Some(mut state) = self.state.lock_mut() {
			state.max = track.latest();

			self.publisher.send_message(message::SubscribeOk {
				id: self.msg.id,
				expires: None,
				latest: state.max,
			});

			state.ok = true; // So we sent SubscribeDone on drop
		} else {
			return Err(ServeError::Done.into());
		}

		let serve = SubscribedServe::new(self.publisher.clone(), self.state.clone(), &self.msg);

		match track.mode().await? {
			TrackReaderMode::Stream(stream) => return serve.track(stream).await,
			TrackReaderMode::Groups(groups) => serve.groups(groups).await,
			TrackReaderMode::Objects(objects) => serve.objects(objects).await,
			TrackReaderMode::Datagrams(datagrams) => serve.datagrams(datagrams).await,
		}
	}

	pub fn close(self, err: ServeError) -> Result<(), ServeError> {
		let mut state = self.state.lock_mut().ok_or(ServeError::Done)?;
		state.closed = Err(err);
		Ok(())
	}
}

impl<S: webtransport_generic::Session> Drop for Subscribed<S> {
	fn drop(&mut self) {
		let state = self.state.lock();
		let err = state.closed.as_ref().err().cloned().unwrap_or(ServeError::Done);

		if state.ok {
			self.publisher.send_message(message::SubscribeDone {
				id: self.msg.id,
				last: state.max,
				code: err.code(),
				reason: err.to_string(),
			});
		} else {
			self.publisher.send_message(message::SubscribeError {
				id: self.msg.id,
				alias: 0,
				code: err.code(),
				reason: err.to_string(),
			});
		}
	}
}

#[derive(Clone)]
struct SubscribedServe<S: webtransport_generic::Session> {
	publisher: Publisher<S>,
	state: State<SubscribedState>,

	id: u64,
	alias: u64,
}

impl<S: webtransport_generic::Session> SubscribedServe<S> {
	fn new(publisher: Publisher<S>, state: State<SubscribedState>, msg: &message::Subscribe) -> Self {
		Self {
			publisher,
			state,
			id: msg.id,
			alias: msg.track_alias,
		}
	}
}

impl<S: webtransport_generic::Session> SubscribedServe<S> {
	async fn track(self, mut track: serve::StreamReader) -> Result<(), SessionError> {
		let stream = self.publisher.open_uni().await?;

		let mut writer = Writer::new(stream);

		let header: data::Header = data::TrackHeader {
			subscribe_id: self.id,
			track_alias: self.alias,
			send_order: track.priority,
		}
		.into();

		writer.encode(&header).await?;

		log::trace!("sent track header: {:?}", header);

		while let Some(mut group) = track.next().await? {
			while let Some(mut object) = group.next().await? {
				let header = data::TrackObject {
					group_id: object.group_id,
					object_id: object.object_id,
					size: object.size,
				};

				self.state
					.lock_mut()
					.ok_or(ServeError::Done)?
					.update_max(object.group_id, object.object_id)?;

				writer.encode(&header).await?;

				log::trace!("sent track object: {:?}", header);

				while let Some(chunk) = object.read().await? {
					writer.write(&chunk).await?;
					log::trace!("sent track payload: {:?}", chunk.len());
				}

				log::trace!("sent track done");
			}
		}

		Ok(())
	}

	pub async fn groups(self, mut groups: serve::GroupsReader) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();
		let mut done = false;

		loop {
			tokio::select! {
				group = groups.next(), if !done => {
					match group? {
						Some(group) => tasks.push(Self::group(self.clone(), group)),
						None => done = true,
					};
				}
				res = tasks.next(), if !tasks.is_empty() => {
					if let Err(err) = res.unwrap() {
						log::warn!("failed to serve group: {:?}", err);
					}
				},
				else => return Ok(()),
			}
		}
	}

	pub async fn group(self, mut group: serve::GroupReader) -> Result<(), SessionError> {
		let stream = self.publisher.open_uni().await?;
		let mut writer = Writer::new(stream);

		let header: data::Header = data::GroupHeader {
			subscribe_id: self.id,
			track_alias: self.alias,
			group_id: group.group_id,
			send_order: group.priority,
		}
		.into();

		writer.encode(&header).await?;

		log::trace!("sent group: {:?}", header);

		while let Some(mut object) = group.next().await? {
			let header = data::GroupObject {
				object_id: object.object_id,
				size: object.size,
			};

			writer.encode(&header).await?;

			self.state
				.lock_mut()
				.ok_or(ServeError::Done)?
				.update_max(group.group_id, object.object_id)?;

			log::trace!("sent group object: {:?}", header);

			while let Some(chunk) = object.read().await? {
				writer.write(&chunk).await?;
				log::trace!("sent group payload: {:?}", chunk.len());
			}

			log::trace!("sent group done");
		}

		Ok(())
	}

	pub async fn objects(self, mut objects: serve::ObjectsReader) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();
		let mut done = false;

		loop {
			tokio::select! {
				object = objects.next(), if !done => {
					match object? {
						Some(object) => tasks.push(Self::object(self.clone(), object)),
						None => done = true,
					};
				}
				res = tasks.next(), if !tasks.is_empty() => {
					if let Err(err) = res.unwrap() {
						log::warn!("failed to serve object: {:?}", err);
					}
				},
				else => return Ok(()),
			}
		}
	}

	pub async fn object(self, mut object: serve::ObjectReader) -> Result<(), SessionError> {
		let stream = self.publisher.open_uni().await?;
		let mut writer = Writer::new(stream);

		let header: data::Header = data::ObjectHeader {
			subscribe_id: self.id,
			track_alias: self.alias,
			group_id: object.group_id,
			object_id: object.object_id,
			send_order: object.priority,
		}
		.into();

		writer.encode(&header).await?;

		log::trace!("sent object: {:?}", header);

		while let Some(chunk) = object.read().await? {
			writer.write(&chunk).await?;
			log::trace!("sent object payload: {:?}", chunk.len());
		}

		self.state
			.lock_mut()
			.ok_or(ServeError::Done)?
			.update_max(object.group_id, object.object_id)?;

		log::trace!("sent object done");

		Ok(())
	}

	pub async fn datagrams(self, mut datagrams: serve::DatagramsReader) -> Result<(), SessionError> {
		while let Some(datagram) = datagrams.read().await? {
			let datagram = data::Datagram {
				subscribe_id: self.id,
				track_alias: self.alias,
				group_id: datagram.group_id,
				object_id: datagram.object_id,
				send_order: datagram.priority,
				payload: datagram.payload,
			};

			let mut buffer = bytes::BytesMut::with_capacity(datagram.payload.len() + 100);
			datagram.encode(&mut buffer)?;

			self.publisher.send_datagram(buffer.into())?;
			log::trace!("sent datagram: {:?}", datagram);

			self.state
				.lock_mut()
				.ok_or(ServeError::Done)?
				.update_max(datagram.group_id, datagram.object_id)?;
		}

		Ok(())
	}
}

pub(super) struct SubscribedRecv {
	_state: State<SubscribedState>,
}

impl SubscribedRecv {
	pub fn recv_unsubscribe(&mut self) -> Result<(), ServeError> {
		// TODO properly cancel
		Ok(())
	}
}
