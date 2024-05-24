use std::ops;

use futures::stream::FuturesUnordered;
use futures::StreamExt;

use crate::serve::ServeError;
use crate::watch::State;
use crate::{data, message, serve};

use super::{Publisher, SessionError, SubscribeInfo, Writer};

#[derive(Debug)]
struct SubscribedState {
	max: Option<(u64, u64)>,
	closed: Result<(), ServeError>,
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
			max: None,
			closed: Ok(()),
		}
	}
}

pub struct Subscribed {
	publisher: Publisher,
	state: State<SubscribedState>,
	msg: message::Subscribe,
	ok: bool,

	pub info: SubscribeInfo,
}

impl Subscribed {
	pub(super) fn new(publisher: Publisher, msg: message::Subscribe) -> (Self, SubscribedRecv) {
		let (send, recv) = State::default().split();
		let info = SubscribeInfo {
			namespace: msg.track_namespace.clone(),
			name: msg.track_name.clone(),
		};

		let send = Self {
			publisher,
			state: send,
			msg,
			info,
			ok: false,
		};

		// Prevents updates after being closed
		let recv = SubscribedRecv { state: recv };

		(send, recv)
	}

	pub async fn serve(mut self, track: serve::TrackReader) -> Result<(), SessionError> {
		let res = self.serve_inner(track).await;
		if let Err(err) = &res {
			self.close(err.clone().into())?;
		}

		res
	}

	async fn serve_inner(&mut self, track: serve::TrackReader) -> Result<(), SessionError> {
		let latest = track.latest();
		self.state.lock_mut().ok_or(ServeError::Cancel)?.max = latest;

		self.publisher.send_message(message::SubscribeOk {
			id: self.msg.id,
			expires: None,
			latest,
		});

		self.ok = true; // So we sent SubscribeDone on drop

		self.serve_groups(track).await
	}

	pub fn close(self, err: ServeError) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(ServeError::Done)?;
		state.closed = Err(err);

		Ok(())
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

impl ops::Deref for Subscribed {
	type Target = SubscribeInfo;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

impl Drop for Subscribed {
	fn drop(&mut self) {
		let state = self.state.lock();
		let err = state.closed.as_ref().err().cloned().unwrap_or(ServeError::Done);
		let max = state.max;
		drop(state); // Important to avoid a deadlock

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

impl Subscribed {
	async fn serve_groups(&mut self, mut groups: serve::TrackReader) -> Result<(), SessionError> {
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
								log::warn!("failed to serve group: {:?}, error: {}", info, err);
							}
						});
					},
					Ok(None) => done = Some(Ok(())),
					Err(err) => done = Some(Err(err)),
				},
				res = self.closed(), if done.is_none() => done = Some(res),
				_ = tasks.next(), if !tasks.is_empty() => {},
				else => return Ok(done.unwrap()?),
			}
		}
	}

	async fn serve_group(
		header: data::GroupHeader,
		mut group: serve::GroupReader,
		mut publisher: Publisher,
		state: State<SubscribedState>,
	) -> Result<(), SessionError> {
		let mut stream = publisher.open_uni().await?;

		// TODO figure out u32 vs u64 priority
		stream.set_priority(group.priority as i32);

		let mut writer = Writer::new(stream);
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
}

pub(super) struct SubscribedRecv {
	state: State<SubscribedState>,
}

impl SubscribedRecv {
	pub fn recv_unsubscribe(&mut self) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		if let Some(mut state) = state.into_mut() {
			state.closed = Err(ServeError::Cancel);
		}

		Ok(())
	}
}
