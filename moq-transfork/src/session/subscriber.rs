use std::{
	collections::HashMap,
	sync::{atomic, Arc, Mutex},
};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	coding::{Reader, Stream},
	message, setup,
	util::Queue,
	Broadcast, BroadcastReader, Closed, Track, TrackReader, TrackWriter, UnknownReader, UnknownWriter,
};

use super::{Session, SessionError, Subscribe};

#[derive(Clone)]
pub struct Subscriber {
	session: web_transport::Session,
	announced: Queue<BroadcastReader>,
	subscribe: Queue<Subscribe>,

	// Used to forward any Subscribes from Announced broadcasts.
	unknown: UnknownReader,

	lookup: Arc<Mutex<HashMap<u64, TrackWriter>>>,
	next_id: Arc<atomic::AtomicU64>,
}

impl Subscriber {
	pub(super) fn new(session: web_transport::Session, unknown: UnknownReader) -> Self {
		Self {
			session,
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

	#[tracing::instrument("session", skip_all, fields(session = self.session.id))]
	pub async fn subscribe(&mut self, track: Track) -> Result<TrackReader, SessionError> {
		let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);
		let (writer, reader) = track.produce();

		self.lookup.lock().unwrap().insert(id, writer);
		let subscribe = Subscribe::open(&mut self.session, id, reader.clone()).await?;

		self.subscribe.push(subscribe).map_err(|_| SessionError::Internal)?;

		Ok(reader)
	}

	pub(super) async fn run(self, mut unknown: UnknownWriter) -> Result<(), SessionError> {
		let mut subscribes = FuturesUnordered::new();
		let mut unknowns = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(subscribe) = self.subscribe.pop() => subscribes.push(async move {
					let id = subscribe.id;
					let _ = subscribe.run().await;
					id
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

	pub(super) async fn recv_announce(&mut self, mut stream: Stream) {
		if let Err(err) = self.recv_announce_inner(&mut stream).await {
			stream.writer.reset(err.code());
		}
	}

	#[tracing::instrument("announced", skip_all, err, fields(stream = stream.id))]
	async fn recv_announce_inner(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let msg = stream.reader.decode::<message::Announce>().await?;

		let (mut writer, reader) = Broadcast::new(&msg.broadcast).produce();
		let _ = writer.unknown(self.unknown.clone());

		self.announced.push(reader).map_err(|_| SessionError::Internal)?;

		tracing::info!(broadcast = writer.name);

		// Send the OK message.
		let msg = message::AnnounceOk {};
		stream.writer.encode(&msg).await?;

		// Wait until the reader is closed.
		tokio::select! {
			res = stream.reader.closed() => res?,
			res = writer.closed() => res?,
		};

		Ok(())
	}

	pub(super) async fn recv_group(&mut self, mut stream: Reader) {
		if let Err(err) = self.recv_group_inner(&mut stream).await {
			stream.stop(err.code());
		}
	}

	#[tracing::instrument("group", skip_all, err, fields(stream = stream.id))]
	async fn recv_group_inner(&mut self, stream: &mut Reader) -> Result<(), SessionError> {
		let _header = message::Data::Group = stream.decode().await?;

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
