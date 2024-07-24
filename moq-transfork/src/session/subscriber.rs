use std::{collections::HashMap, sync::atomic};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	message,
	model::{Broadcast, BroadcastReader, Closed, Produce, Router, Track, TrackReader, TrackWriter},
	runtime::{Lock, Queue, Ref},
};

use super::{Reader, Session, SessionError, Stream, Subscribe};

#[derive(Clone)]
pub struct Subscriber {
	session: Session,
	announced: Queue<BroadcastReader>,
	subscribe: Queue<Subscribe>,

	lookup: Lock<HashMap<u64, TrackWriter>>,
	next_id: Ref<atomic::AtomicU64>, // TODO move to runtime
}

impl Subscriber {
	pub(super) fn new(session: Session) -> Self {
		Self {
			session,
			announced: Default::default(),
			subscribe: Default::default(),
			lookup: Default::default(),
			next_id: Default::default(),
		}
	}

	pub async fn announced(&mut self) -> Option<BroadcastReader> {
		self.announced.pop().await
	}

	pub async fn subscribe(&mut self, broadcast: Broadcast, track: Track) -> Result<TrackReader, SessionError> {
		let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);
		self.subscribe_inner(broadcast, track, id).await
	}

	async fn subscribe_inner(
		&mut self,
		broadcast: Broadcast,
		track: Track,
		id: u64,
	) -> Result<TrackReader, SessionError> {
		let (writer, reader) = track.produce();

		self.lookup.lock().insert(id, writer);

		let stream = self.session.open(message::Stream::Subscribe).await?;
		let mut subscribe = Subscribe::new(stream, id, broadcast, reader.clone());
		subscribe.start().await?; // wait for an OK before returning

		self.subscribe.push(subscribe).map_err(|_| SessionError::Internal)?;

		Ok(reader)
	}

	pub(super) async fn run(self) -> Result<(), SessionError> {
		let mut subscribes = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(subscribe) = self.subscribe.pop() => subscribes.push(async move {
					let id = subscribe.id;
					let _ = subscribe.run().await;
					id
				}),
				res = subscribes.next(), if !subscribes.is_empty() => {
					let id = res.unwrap();
					self.lookup.lock().remove(&id);
				}
				else => return Ok(()),
			};
		}
	}

	pub(super) async fn recv_announce(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let announce = stream.reader.decode().await?;
		self.serve_announce(stream, announce).await
	}

	#[tracing::instrument("announced", skip_all, err, fields(broadcast = announce.broadcast))]
	async fn serve_announce(&mut self, stream: &mut Stream, announce: message::Announce) -> Result<(), SessionError> {
		let broadcast = Broadcast::new(announce.broadcast);
		let (mut writer, reader) = broadcast.clone().produce();

		let mut router = Router::produce();
		writer.route(router.1)?;

		self.announced.push(reader).map_err(|_| SessionError::Internal)?;

		// Send the OK message.
		let msg = message::AnnounceOk {};
		stream.writer.encode(&msg).await?;

		tracing::info!("ok");

		let mut tasks = FuturesUnordered::new();

		// Wait until the reader is closed.
		loop {
			tokio::select! {
				Some(req) = router.0.requested() => {
					let mut this = self.clone();
					let broadcast = broadcast.clone();

					let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);
					tasks.push(async move {
						match this.subscribe_inner(broadcast, req.info.clone(), id).await {
							Ok(track) => req.serve(track),
							Err(err) => req.close(Closed::Unknown /* TODO err*/),
						};
					});
				},
				res = stream.reader.closed() => return res,
				res = writer.closed() => return res.map_err(Into::into),
				_ = tasks.next(), if !tasks.is_empty() => {},
			};
		}
	}

	pub(super) async fn recv_group(&mut self, stream: &mut Reader) -> Result<(), SessionError> {
		let group = stream.decode().await?;
		self.serve_group(stream, group).await
	}

	#[tracing::instrument("data", skip_all, err, fields(group = group.sequence))]
	async fn serve_group(&mut self, stream: &mut Reader, group: message::Group) -> Result<(), SessionError> {
		let mut group = self
			.lookup
			.lock()
			.get_mut(&group.subscribe)
			.ok_or(Closed::Unknown)?
			.create(group.sequence)?;

		let mut size = 0;
		while let Some(chunk) = stream.read_chunk(usize::MAX).await? {
			size += chunk.len();
			group.write(chunk)?;
		}

		tracing::debug!(size);

		Ok(())
	}

	pub async fn closed(&self) -> Result<(), SessionError> {
		self.session.closed().await
	}
}
