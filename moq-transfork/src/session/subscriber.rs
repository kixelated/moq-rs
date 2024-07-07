use std::{
	collections::HashMap,
	sync::{atomic, Arc, Mutex},
};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	message, setup, util::Queue, Broadcast, BroadcastReader, Closed, Track, TrackReader, TrackWriter, UnknownReader,
	UnknownWriter,
};

use super::{Reader, Session, SessionError, Stream, Subscribe};

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

	pub(super) async fn recv_announce(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let announce = stream.reader.decode().await?;
		self.serve_announce(stream, announce).await
	}

	#[tracing::instrument("announce", skip_all, err, fields(broadcast = announce.broadcast))]
	async fn serve_announce(&mut self, stream: &mut Stream, announce: message::Announce) -> Result<(), SessionError> {
		let (mut writer, reader) = Broadcast::new(&announce.broadcast).produce();
		let _ = writer.unknown(self.unknown.clone());

		self.announced.push(reader).map_err(|_| SessionError::Internal)?;

		// Send the OK message.
		let msg = message::AnnounceOk {};
		stream.writer.encode(&msg).await?;

		tracing::info!("ok");

		// Wait until the reader is closed.
		tokio::select! {
			res = stream.reader.closed() => res?,
			res = writer.closed() => res?,
		};

		Ok(())
	}

	pub(super) async fn recv_group(&mut self, stream: &mut Reader) -> Result<(), SessionError> {
		let group = stream.decode().await?;
		self.serve_group(stream, group).await
	}

	#[tracing::instrument("group", skip_all, err, fields(broadcast, track, group = group.sequence, subscribe = group.subscribe))]
	async fn serve_group(&mut self, stream: &mut Reader, group: message::Group) -> Result<(), SessionError> {
		let mut group = {
			let mut lookup = self.lookup.lock().unwrap();
			let track = lookup.get_mut(&group.subscribe).ok_or(Closed::UnknownSubscribe)?;

			tracing::Span::current().record("broadcast", &track.broadcast);
			tracing::Span::current().record("track", &track.name);

			track.create(group.sequence)?
		};

		let mut size = 0;
		while let Some(chunk) = stream.read_chunk(usize::MAX).await? {
			size += chunk.len();
			group.write(chunk)?;
		}

		tracing::debug!(size);

		Ok(())
	}
}
