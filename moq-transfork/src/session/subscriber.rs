use std::{
	collections::HashMap,
	sync::{atomic, Arc, Mutex},
};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	message, setup, util::Queue, Broadcast, BroadcastReader, Closed, Produce, Router, Track, TrackReader, TrackWriter,
};

use super::{Reader, Session, SessionError, Stream, Subscribe};

#[derive(Clone)]
pub struct Subscriber {
	session: web_transport::Session,
	announced: Queue<BroadcastReader>,
	subscribe: Queue<Subscribe>,

	lookup: Arc<Mutex<HashMap<u64, (Broadcast, TrackWriter)>>>,
	next_id: Arc<atomic::AtomicU64>,
}

impl Subscriber {
	pub(super) fn new(session: web_transport::Session) -> Self {
		Self {
			session,
			announced: Default::default(),
			subscribe: Default::default(),
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

	#[tracing::instrument("session", skip_all, fields(session = self.session.id))]
	pub async fn subscribe(&mut self, broadcast: Broadcast, track: Track) -> Result<TrackReader, SessionError> {
		let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);
		let (writer, reader) = track.produce();

		self.lookup.lock().unwrap().insert(id, (broadcast.clone(), writer));

		let stream = Stream::open(&mut self.session, message::Stream::Subscribe).await?;
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
					self.lookup.lock().unwrap().remove(&id);
				}
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

					tasks.push(async move {
						match this.subscribe(broadcast, req.info.clone()).await {
							Ok(track) => req.respond(track),
							Err(err) => req.reject(Closed::Unknown /* TODO err*/),
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

	#[tracing::instrument("group", skip_all, err, fields(broadcast, track, group = group.sequence, subscribe = group.subscribe))]
	async fn serve_group(&mut self, stream: &mut Reader, group: message::Group) -> Result<(), SessionError> {
		let mut group = {
			let mut lookup = self.lookup.lock().unwrap();
			let (broadcast, track) = lookup.get_mut(&group.subscribe).ok_or(Closed::Unknown)?;

			tracing::Span::current().record("broadcast", &broadcast.name);
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
