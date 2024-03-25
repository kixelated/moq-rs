use std::sync::Arc;

use futures::stream::FuturesUnordered;
use futures::StreamExt;

use crate::coding::{Encode, Writer};
use crate::serve::{ServeError, TrackReaderMode};
use crate::util::{Watch, WatchWeak};
use crate::{data, message, serve};

use super::{Publisher, SessionError};

#[derive(Clone)]
pub struct Subscribed<S: webtransport_generic::Session> {
	session: Publisher<S>,
	state: Watch<State<S>>,
	msg: message::Subscribe,
}

impl<S: webtransport_generic::Session> Subscribed<S> {
	pub(super) fn new(session: Publisher<S>, msg: message::Subscribe) -> (Subscribed<S>, SubscribedRecv<S>) {
		let state = Watch::new(State::new(session.clone(), msg.id));
		let recv = SubscribedRecv {
			state: state.downgrade(),
		};

		let subscribed = Self { session, state, msg };
		(subscribed, recv)
	}

	pub fn namespace(&self) -> &str {
		self.msg.track_namespace.as_str()
	}

	pub fn name(&self) -> &str {
		self.msg.track_name.as_str()
	}

	pub async fn serve(mut self, track: serve::TrackReader) -> Result<(), SessionError> {
		self.state.lock_mut().ok(track.latest())?;

		match track.mode().await? {
			TrackReaderMode::Stream(stream) => return self.serve_track(stream).await,
			TrackReaderMode::Groups(groups) => self.serve_groups(groups).await,
			TrackReaderMode::Objects(objects) => self.serve_objects(objects).await,
			TrackReaderMode::Datagrams(datagram) => self.serve_datagrams(datagram).await,
		}
	}

	async fn serve_track(mut self, mut track: serve::StreamReader) -> Result<(), SessionError> {
		let stream = self
			.session
			.webtransport()
			.open_uni()
			.await
			.map_err(|e| SessionError::WebTransport(Arc::new(e)))?;

		let mut writer = Writer::new(stream);

		let header: data::Header = data::TrackHeader {
			subscribe_id: self.msg.id,
			track_alias: self.msg.track_alias,
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

				self.state.lock_mut().update_max(object.group_id, object.object_id)?;

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

	pub async fn serve_groups(self, mut groups: serve::GroupsReader) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();
		let mut done = false;

		loop {
			tokio::select! {
				group = groups.next(), if !done => {
					match group? {
						Some(group) => tasks.push(Self::serve_group(self.clone(), group)),
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

	pub async fn serve_group(mut self, mut group: serve::GroupReader) -> Result<(), SessionError> {
		let stream = self
			.session
			.webtransport()
			.open_uni()
			.await
			.map_err(|e| SessionError::WebTransport(Arc::new(e)))?;
		let mut writer = Writer::new(stream);

		let header: data::Header = data::GroupHeader {
			subscribe_id: self.msg.id,
			track_alias: self.msg.track_alias,
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

			self.state.lock_mut().update_max(group.group_id, object.object_id)?;

			log::trace!("sent group object: {:?}", header);

			while let Some(chunk) = object.read().await? {
				writer.write(&chunk).await?;
				log::trace!("sent group payload: {:?}", chunk.len());
			}

			log::trace!("sent group done");
		}

		Ok(())
	}
	pub async fn serve_objects(self, mut objects: serve::ObjectsReader) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();
		let mut done = false;

		loop {
			tokio::select! {
				object = objects.next(), if !done => {
					match object? {
						Some(object) => tasks.push(Self::serve_object(self.clone(), object)),
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

	pub async fn serve_object(mut self, mut object: serve::ObjectReader) -> Result<(), SessionError> {
		let stream = self
			.session
			.webtransport()
			.open_uni()
			.await
			.map_err(|e| SessionError::WebTransport(Arc::new(e)))?;
		let mut writer = Writer::new(stream);

		let header: data::Header = data::ObjectHeader {
			subscribe_id: self.msg.id,
			track_alias: self.msg.track_alias,
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

		self.state.lock_mut().update_max(object.group_id, object.object_id)?;

		log::trace!("sent object done");

		Ok(())
	}

	pub async fn serve_datagrams(&mut self, mut datagrams: serve::DatagramsReader) -> Result<(), SessionError> {
		while let Some(datagram) = datagrams.read().await? {
			let datagram = data::Datagram {
				subscribe_id: self.msg.id,
				track_alias: self.msg.track_alias,
				group_id: datagram.group_id,
				object_id: datagram.object_id,
				send_order: datagram.priority,
				payload: datagram.payload,
			};

			let mut buffer = bytes::BytesMut::with_capacity(datagram.payload.len() + 100);
			datagram.encode(&mut buffer)?;

			self.session
				.webtransport()
				.send_datagram(buffer.into())
				.map_err(|e| SessionError::WebTransport(Arc::new(e)))?;
			log::trace!("sent datagram: {:?}", datagram);

			self.state
				.lock_mut()
				.update_max(datagram.group_id, datagram.object_id)?;
		}

		Ok(())
	}

	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.state.lock_mut().close(err)
	}

	pub async fn closed(&self) -> Result<(), ServeError> {
		loop {
			let notify = {
				let state = self.state.lock();
				state.closed.clone()?;
				state.changed()
			};

			notify.await
		}
	}
}

pub(super) struct SubscribedRecv<S: webtransport_generic::Session> {
	state: WatchWeak<State<S>>,
}

impl<S: webtransport_generic::Session> SubscribedRecv<S> {
	pub fn recv_unsubscribe(&mut self) -> Result<(), ServeError> {
		if let Some(state) = self.state.upgrade() {
			state.lock_mut().close(ServeError::Done)?;
		}
		Ok(())
	}
}

struct State<S: webtransport_generic::Session> {
	session: Publisher<S>,
	id: u64,

	ok: bool,
	max: Option<(u64, u64)>,
	closed: Result<(), ServeError>,
}

impl<S: webtransport_generic::Session> State<S> {
	fn new(session: Publisher<S>, id: u64) -> Self {
		Self {
			session,
			id,
			ok: false,
			max: None,
			closed: Ok(()),
		}
	}
}

impl<S: webtransport_generic::Session> State<S> {
	fn ok(&mut self, latest: Option<(u64, u64)>) -> Result<(), ServeError> {
		self.ok = true;
		self.max = latest;

		self.session
			.send_message(message::SubscribeOk {
				id: self.id,
				expires: None,
				latest,
			})
			.ok();

		Ok(())
	}

	fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.closed.clone()?;
		self.closed = Err(err.clone());

		if self.ok {
			self.session
				.send_message(message::SubscribeDone {
					id: self.id,
					last: self.max,
					code: err.code(),
					reason: err.to_string(),
				})
				.ok();
		} else {
			self.session
				.send_message(message::SubscribeError {
					id: self.id,
					alias: 0,
					code: err.code(),
					reason: err.to_string(),
				})
				.ok();
		}

		Ok(())
	}

	fn update_max(&mut self, group_id: u64, object_id: u64) -> Result<(), ServeError> {
		self.closed.clone()?;

		if let Some((max_group, max_object)) = self.max {
			if group_id >= max_group && object_id >= max_object {
				self.max = Some((group_id, object_id));
			}
		}

		Ok(())
	}
}

impl<S: webtransport_generic::Session> Drop for State<S> {
	fn drop(&mut self) {
		self.close(ServeError::Done).ok();
	}
}
