use std::sync::Arc;

use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};

use crate::coding::{Encode, Writer};
use crate::serve::ServeError;
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

	pub async fn serve(mut self, mut track: serve::TrackSubscriber) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		self.state.lock_mut().ok(track.latest())?;
		let mut done = false;

		loop {
			tokio::select! {
				next = track.next(), if !done => {
					let next = match next? {
						Some(next) => next,
						None => { done = true; continue },
					};

					match next {
						serve::TrackMode::Stream(stream) => return self.serve_track(stream).await,
						serve::TrackMode::Group(group) => tasks.push(Self::serve_group(self.clone(), group).boxed()),
						serve::TrackMode::Object(object) => tasks.push(Self::serve_object(self.clone(), object).boxed()),
						serve::TrackMode::Datagram(datagram) => self.serve_datagram(datagram).await?,
					}
				},
				task = tasks.next(), if !tasks.is_empty() => task.unwrap()?,
				else => return Ok(()),
			};
		}
	}

	async fn serve_track(mut self, mut track: serve::StreamSubscriber) -> Result<(), SessionError> {
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
			send_order: track.send_order,
		}
		.into();

		writer.encode(&header).await?;

		log::trace!("sent track header: {:?}", header);

		while let Some(object) = track.next().await? {
			// TODO support streaming chunks
			// TODO check if closed

			let header = data::TrackObject {
				group_id: object.group_id,
				object_id: object.object_id,
				size: object.payload.len(),
			};

			writer.encode(&header).await?;

			log::trace!("sent track object: {:?}", header);

			self.state.lock_mut().update_max(object.group_id, object.object_id)?;
			writer.write(&object.payload).await?;

			log::trace!("sent track payload: {:?}", object.payload.len());
			log::trace!("sent track done");
		}

		Ok(())
	}

	pub async fn serve_group(mut self, mut group: serve::GroupSubscriber) -> Result<(), SessionError> {
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
			group_id: group.id,
			send_order: group.send_order,
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

			self.state.lock_mut().update_max(group.id, object.object_id)?;

			log::trace!("sent group object: {:?}", header);

			while let Some(chunk) = object.read().await? {
				writer.write(&chunk).await?;
				log::trace!("sent group payload: {:?}", chunk.len());
			}

			log::trace!("sent group done");
		}

		Ok(())
	}

	pub async fn serve_object(mut self, mut object: serve::ObjectSubscriber) -> Result<(), SessionError> {
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
			send_order: object.send_order,
		}
		.into();

		writer.encode(&header).await?;

		log::trace!("sent object: {:?}", header);

		self.state.lock_mut().update_max(object.group_id, object.object_id)?;

		while let Some(chunk) = object.read().await? {
			writer.write(&chunk).await?;
			log::trace!("sent object payload: {:?}", chunk.len());
		}

		log::trace!("sent object done");

		Ok(())
	}

	pub async fn serve_datagram(&mut self, datagram: serve::Datagram) -> Result<(), SessionError> {
		let datagram = data::Datagram {
			subscribe_id: self.msg.id,
			track_alias: self.msg.track_alias,
			group_id: datagram.group_id,
			object_id: datagram.object_id,
			send_order: datagram.send_order,
			payload: datagram.payload,
		};

		let mut buffer = Vec::with_capacity(datagram.payload.len() + 100);
		datagram.encode(&mut buffer)?;

		log::trace!("sent datagram: {:?}", datagram);

		// TODO send the datagram
		//self.session.webtransport().send_datagram(&buffer)?;

		self.state
			.lock_mut()
			.update_max(datagram.group_id, datagram.object_id)?;

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
		self.session.drop_subscribe(self.id);
	}
}
