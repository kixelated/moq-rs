use std::{
	collections::HashMap,
	sync::{atomic, Arc, Mutex},
};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	coding::{Reader, Stream},
	message, setup, Broadcast, BroadcastReader, BroadcastWriter, Closed, Track, TrackReader, TrackWriter,
	UnknownReader, UnknownWriter,
};

use crate::util::Queue;

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
		let mut control = Stream::open(&mut self.webtransport, message::Control::Subscribe).await?;

		let (id, reader) = match self.subscribe_start(track, &mut control).await {
			Ok(v) => v,
			Err(err) => {
				control.writer.reset(err.code());
				return Err(err);
			}
		};

		let subscribe = Subscribe::new(id, control, reader.clone());
		if self.subscribe.push(subscribe).is_err() {
			return Err(SessionError::Internal);
		}

		Ok(reader)
	}

	async fn subscribe_start(
		&mut self,
		track: Track,
		control: &mut Stream,
	) -> Result<(u64, TrackReader), SessionError> {
		let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);

		let msg = message::Subscribe {
			id,
			broadcast: track.broadcast.to_string(),

			track: track.name.clone(),
			priority: track.priority,

			group_order: track.group_order,
			group_expires: track.group_expires,

			// TODO
			group_min: None,
			group_max: None,
		};

		control.writer.encode(&msg).await?;

		// TODO use the response to update the track
		let info: message::Info = control.reader.decode().await?;

		let (writer, reader) = track.produce();
		self.lookup.lock().unwrap().insert(id, writer);

		Ok((id, reader))
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

	pub(super) async fn run_announce(&mut self, control: &mut Stream) -> Result<(), SessionError> {
		let msg: message::Announce = control.reader.decode().await?;
		let (mut writer, reader) = Broadcast::new(&msg.broadcast).produce();
		writer.unknown(self.unknown.clone())?;

		let announced = Announced::new(writer);
		let _ = self.announced.push(reader);

		announced.run(control).await
	}

	pub(super) async fn run_group(&mut self, reader: &mut Reader) -> Result<(), SessionError> {
		let header: message::Group = reader.decode().await?;

		let mut group = self
			.lookup
			.lock()
			.unwrap()
			.get_mut(&header.subscribe)
			.ok_or(Closed::UnknownSubscribe)?
			.create(header.sequence)?;

		while let Some(chunk) = reader.read_chunk(usize::MAX).await? {
			group.write(chunk)?;
		}

		Ok(())
	}
}
