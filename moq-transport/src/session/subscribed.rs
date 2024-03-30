use std::ops;

use futures::stream::FuturesUnordered;
use futures::StreamExt;

use webtransport_generic::SendStream;

use crate::coding::Encode;
use crate::serve::{ServeError, TrackReaderMode};
use crate::util::State;
use crate::{data, message, serve, Publisher};

use super::{SessionError, SubscribeInfo, Writer};

#[derive(Debug)]
struct SubscribedState {
	max: Option<(u64, u64)>,
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
		Self { max: None }
	}
}

pub struct Subscribed<S: webtransport_generic::Session> {
	publisher: Publisher<S>,
	state: State<SubscribedState>,
	msg: message::Subscribe,

	closed: Result<(), ServeError>,
	ok: bool,

	pub info: SubscribeInfo,
}

impl<S: webtransport_generic::Session> Subscribed<S> {
	pub(super) fn new(publisher: Publisher<S>, msg: message::Subscribe) -> (Self, SubscribedRecv) {
		let (send, recv) = State::default();
		let info = SubscribeInfo {
			namespace: msg.track_namespace.clone(),
			name: msg.track_name.clone(),
		};

		let send = Self {
			publisher,
			state: send,
			msg,
			info,
			closed: Ok(()),
			ok: false,
		};

		// Prevents updates after being closed
		// TODO actively abort on close
		let recv = SubscribedRecv { _state: recv };

		(send, recv)
	}

	pub async fn serve(mut self, track: serve::TrackReader) -> Result<(), SessionError> {
		self.ok = true; // So we sent SubscribeDone on drop

		let latest = track.latest();
		self.state.lock_mut().ok_or(ServeError::Done)?.max = latest;

		self.publisher.send_message(message::SubscribeOk {
			id: self.msg.id,
			expires: None,
			latest,
		});

		match track.mode().await? {
			TrackReaderMode::Stream(stream) => self.serve_track(stream).await,
			TrackReaderMode::Groups(groups) => self.serve_groups(groups).await,
			TrackReaderMode::Objects(objects) => self.serve_objects(objects).await,
			TrackReaderMode::Datagrams(datagrams) => self.serve_datagrams(datagrams).await,
		}
	}

	pub fn close(mut self, err: ServeError) -> Result<(), ServeError> {
		self.closed = Err(err);
		Ok(())
	}
}

impl<S: webtransport_generic::Session> ops::Deref for Subscribed<S> {
	type Target = SubscribeInfo;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

impl<S: webtransport_generic::Session> Drop for Subscribed<S> {
	fn drop(&mut self) {
		// WARN: there's a deadlock if you hold the lock while sending a terminal message...
		let err = self.closed.as_ref().err().cloned().unwrap_or(ServeError::Done);
		let max = self.state.lock().max;

		if self.ok {
			self.publisher.send_message(message::SubscribeDone {
				id: self.msg.id,
				last: max,
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
		};
	}
}

impl<S: webtransport_generic::Session> Subscribed<S> {
	async fn serve_track(self, mut track: serve::StreamReader) -> Result<(), SessionError> {
		let mut stream = self.publisher.open_uni().await?;

		// TODO figure out u32 vs u64 priority
		stream.priority(track.priority as i32);

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

	async fn serve_groups(self, mut groups: serve::GroupsReader) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();
		let mut done: Option<Result<(), ServeError>> = None;

		loop {
			tokio::select! {
				res = groups.next(), if done.is_none() => match res {
					Ok(Some(group)) => {
						let header = data::GroupHeader {
							subscribe_id: self.msg.id,
							track_alias: self.msg.track_alias,
							group_id: group.group_id,
							send_order: group.priority,
						};

						let publisher = self.publisher.clone();
						let state = self.state.clone();
						let info = group.info.clone();

						tasks.push(async move {
							if let Err(err) = Self::serve_group(header, group, publisher, state).await {
								log::warn!("failed to serve group: group={:?} err={:?}", info, err);
							}
						});
					},
					Ok(None) => done = Some(Ok(())),
					Err(err) => done = Some(Err(err.into())),
				},
				_ = tasks.select_next_some() => {},
			}
		}
	}

	async fn serve_group(
		header: data::GroupHeader,
		mut group: serve::GroupReader,
		publisher: Publisher<S>,
		state: State<SubscribedState>,
	) -> Result<(), SessionError> {
		let mut stream = publisher.open_uni().await?;

		// TODO figure out u32 vs u64 priority
		stream.priority(group.priority as i32);

		let mut writer = Writer::new(stream);

		let header: data::Header = header.into();
		writer.encode(&header).await?;

		log::trace!("sent group: {:?}", header);

		while let Some(mut object) = group.next().await? {
			let header = data::GroupObject {
				object_id: object.object_id,
				size: object.size,
			};

			writer.encode(&header).await?;

			state
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

	pub async fn serve_objects(self, mut objects: serve::ObjectsReader) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();
		let mut done = None;

		loop {
			tokio::select! {
				res = objects.next(), if done.is_none() => match res {
					Ok(Some(object)) => {
						let header = data::ObjectHeader {
							subscribe_id: self.msg.id,
							track_alias: self.msg.track_alias,
							group_id: object.group_id,
							object_id: object.object_id,
							send_order: object.priority,
						};

						let publisher = self.publisher.clone();
						let state = self.state.clone();
						let info = object.info.clone();

						tasks.push(async move {
							if let Err(err) = Self::serve_object(header, object, publisher, state).await {
								log::warn!("failed to serve object: object={:?} err={:?}", info, err);
							};
						});
					},
					Ok(None) => done = Some(Ok(())),
					Err(err) => done = Some(Err(err.into())),
				},
				_ = tasks.select_next_some() => {},
				else => return done.unwrap(),
			}
		}
	}

	async fn serve_object(
		header: data::ObjectHeader,
		mut object: serve::ObjectReader,
		publisher: Publisher<S>,
		state: State<SubscribedState>,
	) -> Result<(), SessionError> {
		state
			.lock_mut()
			.ok_or(ServeError::Done)?
			.update_max(object.group_id, object.object_id)?;

		let mut stream = publisher.open_uni().await?;

		// TODO figure out u32 vs u64 priority
		stream.priority(object.priority as i32);

		let mut writer = Writer::new(stream);

		let header: data::Header = header.into();
		writer.encode(&header).await?;

		log::trace!("sent object: {:?}", header);

		while let Some(chunk) = object.read().await? {
			writer.write(&chunk).await?;
			log::trace!("sent object payload: {:?}", chunk.len());
		}

		log::trace!("sent object done");

		Ok(())
	}

	async fn serve_datagrams(self, mut datagrams: serve::DatagramsReader) -> Result<(), SessionError> {
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
