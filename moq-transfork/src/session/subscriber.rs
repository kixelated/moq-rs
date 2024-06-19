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

	pub fn subscribe(&mut self, track: Track) -> TrackReader {
		let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);

		let msg = message::Subscribe {
			id,
			broadcast: track.broadcast.to_string(),

			track: track.name.clone(),
			track_priority: track.priority.unwrap_or(0),

			// TODO
			group_order: track.group_order.map(Into::into),
			group_expires: None,
			group_min: None,
			group_max: None,
		};

		let (writer, reader) = track.produce();
		let subscribe = Subscribe::new(msg, reader.clone());

		self.lookup.lock().unwrap().insert(id, writer);
		let _ = self.subscribe.push(subscribe);

		reader
	}

	pub(super) async fn run(mut self, mut unknown: UnknownWriter) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(mut subscribe) = self.subscribe.pop() => {
					let this = self.clone();
					tasks.push(async move {
						// TODO error handling
						let res = subscribe.run(this.webtransport).await;
						this.lookup.lock().unwrap().remove(&subscribe.id);
						res
					});
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?,
				Some(unknown) = unknown.requested() => {
					let track = self.subscribe(unknown.track.clone());
					unknown.respond(track);
				},
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
			.ok_or(Closed::NotFound)?
			.create(header.sequence)?;

		while let Some(chunk) = reader.read_chunk(usize::MAX).await? {
			group.write(chunk)?;
		}

		Ok(())
	}
}
