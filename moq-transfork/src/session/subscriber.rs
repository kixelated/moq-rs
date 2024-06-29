use std::{
	collections::HashMap,
	fmt,
	sync::{atomic, Arc, Mutex},
};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	coding::{Reader, Stream},
	message, setup,
	util::Queue,
	Broadcast, BroadcastReader, Closed, Track, TrackReader, TrackWriter, UnknownReader, UnknownWriter,
};

use super::{Announced, Session, SessionError, Subscribe};

#[derive(Clone)]
pub struct Subscriber {
	webtransport: web_transport::Session,
	announced: Queue<BroadcastReader>,
	subscribe: Queue<Subscribe>,

	// Used to forward any Subscribes from Announced broadcasts.
	unknown: UnknownReader,

	lookup: Arc<Mutex<HashMap<u64, TrackWriter>>>,
	next_id: Arc<atomic::AtomicU64>,
}

impl Subscriber {
	pub(super) fn new(webtransport: web_transport::Session, unknown: UnknownReader) -> Self {
		Self {
			webtransport,
			announced: Default::default(),
			subscribe: Default::default(),
			unknown,
			lookup: Default::default(),
			next_id: Default::default(),
		}
	}

	pub async fn accept(session: web_transport::Session) -> Result<(Session, Self), SessionError> {
		let (session, _, subscriber) = Session::accept_role(session, setup::Role::Subscriber).await?;
		Ok((session, subscriber.unwrap()))
	}

	pub async fn connect(session: web_transport::Session) -> Result<(Session, Self), SessionError> {
		let (session, _, subscriber) = Session::connect_role(session, setup::Role::Subscriber).await?;
		Ok((session, subscriber.unwrap()))
	}

	pub async fn announced(&mut self) -> Option<BroadcastReader> {
		self.announced.pop().await
	}

	// Manually route a broadcast to this subscriber.
	pub fn route(&mut self, broadcast: Broadcast) -> Result<BroadcastReader, SessionError> {
		let (mut writer, reader) = broadcast.produce();
		writer.unknown(self.unknown.clone())?;
		Ok(reader)
	}

	pub async fn subscribe(&mut self, track: Track) -> Result<TrackReader, SessionError> {
		let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);

		let stream = Stream::open(&mut self.webtransport, message::Control::Subscribe).await?;
		let (writer, reader) = track.produce();

		self.lookup.lock().unwrap().insert(id, writer);
		let mut subscribe = Subscribe::new(id, stream, reader.clone());

		subscribe.start().await?;

		if self.subscribe.push(subscribe).is_err() {
			return Err(SessionError::Internal);
		}

		Ok(reader)
	}

	pub(super) async fn run(self, mut unknown: UnknownWriter) -> Result<(), SessionError> {
		let mut subscribes = FuturesUnordered::new();
		let mut unknowns = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(mut subscribe) = self.subscribe.pop() => subscribes.push(async move {
					subscribe.run().await;
					subscribe.id
				}),
				res = subscribes.next(), if !subscribes.is_empty() => {
					let id = res.unwrap();
					self.lookup.lock().unwrap().remove(&id);
				}
				Some(unknown) = unknown.requested() => {
					let mut this = self.clone();

					unknowns.push(async move {
						match this.subscribe(unknown.track.clone()).await {
							Ok(track) => unknown.respond(track),
							Err(SessionError::Closed(err)) => unknown.close(err),
							Err(err) => return Err(err),
						};

						Ok(())
					})
				},
				res = unknowns.next(), if !unknowns.is_empty() => res.unwrap()?,
				else => return Ok(()),
			};
		}
	}

	#[tracing::instrument("recv_announce", skip(self), fields(session = self.webtransport.id(), stream = stream.id()))]
	pub(super) async fn recv_announce(&mut self, mut stream: Stream) {
		if let Err(err) = self.recv_announce_inner(&mut stream).await {
			tracing::warn!(?err);
			stream.close(err.code());
		}
	}

	async fn recv_announce_inner(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let announce: message::Announce = stream.reader.decode().await?;
		tracing::debug!(?announce);

		let (mut writer, reader) = Broadcast::new(&announce.broadcast).produce();
		writer.unknown(self.unknown.clone())?;

		let announced = Announced::new(writer);
		let _ = self.announced.push(reader);

		announced.run(stream).await
	}

	#[tracing::instrument("recv_group", skip(self), fields(session = self.webtransport.id(), stream = stream.id()))]
	pub(super) async fn recv_group(&mut self, mut stream: Reader) {
		if let Err(err) = self.recv_group_inner(&mut stream).await {
			tracing::warn!(?err);
			stream.stop(err.code());
		}
	}

	pub(super) async fn recv_group_inner(&mut self, stream: &mut Reader) -> Result<(), SessionError> {
		let header: message::Group = stream.decode().await?;
		tracing::debug!(?header);

		let mut group = self
			.lookup
			.lock()
			.unwrap()
			.get_mut(&header.subscribe)
			.ok_or(Closed::UnknownSubscribe)?
			.create(header.sequence)?;

		while let Some(chunk) = stream.read_chunk(usize::MAX).await? {
			tracing::trace!(chunk.size = chunk.len());
			group.write(chunk)?;
		}

		Ok(())
	}
}
