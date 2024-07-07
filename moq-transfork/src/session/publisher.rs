use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	message, setup, util::Queue, BroadcastReader, Closed, GroupReader, Track, TrackReader, Unknown, UnknownReader,
	UnknownWriter,
};

use super::{Announce, OrClose, Session, SessionError, Stream, Writer};

#[derive(Clone)]
pub struct Publisher {
	session: web_transport::Session,

	// Used to route incoming subscriptions
	broadcasts: Arc<Mutex<HashMap<String, BroadcastReader>>>,

	announced: Queue<Announce>,
	unknown: Option<UnknownReader>,
}

impl Publisher {
	pub(crate) fn new(webtransport: web_transport::Session) -> Self {
		Self {
			session: webtransport,
			broadcasts: Default::default(),
			announced: Default::default(),
			unknown: None,
		}
	}

	pub async fn accept(session: web_transport::Session) -> Result<(Session, Publisher), SessionError> {
		let (session, publisher, _) = Session::accept_role(session, setup::Role::Publisher).await?;
		Ok((session, publisher.unwrap()))
	}

	pub async fn connect(session: web_transport::Session) -> Result<(Session, Publisher), SessionError> {
		let (session, publisher, _) = Session::connect_role(session, setup::Role::Publisher).await?;
		Ok((session, publisher.unwrap()))
	}

	/// Announce a broadcast and serve tracks using the returned [BroadcastWriter].
	#[tracing::instrument("session", skip_all, fields(id = self.session.id))]
	pub async fn announce(&mut self, broadcast: BroadcastReader) -> Result<(), SessionError> {
		match self.broadcasts.lock().unwrap().entry(broadcast.name.clone()) {
			hash_map::Entry::Occupied(_) => return Err(Closed::Duplicate.into()),
			hash_map::Entry::Vacant(entry) => entry.insert(broadcast.clone()),
		};

		let announce = Announce::open(&mut self.session, broadcast).await?;
		self.announced.push(announce).map_err(|_| SessionError::Internal)?;

		Ok(())
	}

	/// Route any unknown subscriptions to the provided [UnknownWriter].
	///
	/// If this is not called, any uknonwn subscriptions will be rejected with [ServeError::NotFound].
	/// This may be called multiple times, but only the last one will be used.
	pub fn unknown(&mut self) -> UnknownWriter {
		let (writer, reader) = Unknown::produce();
		self.unknown = Some(reader);
		writer
	}

	pub(super) async fn run(self) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(announce) = self.announced.pop() => {
					tasks.push(async move {
						let id = announce.id().to_string();
						let _ = announce.run().await;
						id
					});
				},
				res = tasks.next(), if !tasks.is_empty() => {
					let announce = res.unwrap();
					self.broadcasts.lock().unwrap().remove(&announce);
				},
			}
		}
	}

	async fn subscribe(&mut self, track: Track) -> Result<TrackReader, Closed> {
		let broadcast = self.broadcasts.lock().unwrap().get(&track.broadcast).cloned();
		if let Some(mut broadcast) = broadcast {
			return broadcast.subscribe(track).await;
		}

		if let Some(unknown) = self.unknown.as_mut() {
			return unknown.subscribe(track).await;
		}

		Err(Closed::UnknownBroadcast)
	}

	pub(super) async fn recv_subscribe(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let subscribe = stream.reader.decode().await?;
		self.serve_subscribe(stream, subscribe).await
	}

	#[tracing::instrument("subscribe", skip_all, err, fields(broadcast = subscribe.broadcast, track = subscribe.track, subscribe = subscribe.id))]
	async fn serve_subscribe(
		&mut self,
		stream: &mut Stream,
		subscribe: message::Subscribe,
	) -> Result<(), SessionError> {
		let track = Track::new(&subscribe.broadcast, &subscribe.track, subscribe.priority).build();
		let mut track = self.subscribe(track).await?;

		let info = message::Info {
			group_latest: track.latest(),
			group_expires: track.group_expires,
			group_order: track.group_order,
			track_priority: track.priority,
		};

		stream.writer.encode(&info).await?;

		tracing::info!("ok");

		let mut tasks = FuturesUnordered::new();
		let mut fin = false;

		loop {
			tokio::select! {
				res = track.next(), if !fin => {
					let mut group = match res? {
						Some(group) => group,
						None => {
							fin = true;
							continue;
						},
					};

					let session = self.session.clone();
					let broadcast= track.broadcast.clone();
					let track = track.name.clone();

					tasks.push(async move {
						let res = Self::serve_group(session, broadcast, track, subscribe.id, &mut group).await;
						(group, res)
					});
				},
				res = stream.reader.decode_maybe::<message::SubscribeUpdate>() => {
					match res? {
						Some(_update) => {
							// TODO use it
						},
						None => return Ok(()),
					}
				},
				res = tasks.next(), if !tasks.is_empty() => {
					let (group, res) = res.unwrap();

					if let Err(err) = res {
						let drop = message::GroupDrop {
							sequence: group.sequence,
							count: 0,
							code: err.code(),
						};

						stream.writer.encode(&drop).await?;
					}
				},
			}
		}
	}

	#[tracing::instrument("group", skip_all, err, fields(?broadcast, ?track, subscribe, group = group.sequence))]
	pub async fn serve_group(
		mut session: web_transport::Session,
		broadcast: String,
		track: String,
		subscribe: u64,
		group: &mut GroupReader,
	) -> Result<(), SessionError> {
		let mut stream = Writer::open(&mut session, message::StreamUni::Group).await?;

		Self::serve_group_inner(subscribe, group, &mut stream)
			.await
			.or_close(&mut stream)
	}

	pub async fn serve_group_inner(
		subscribe: u64,
		group: &mut GroupReader,
		stream: &mut Writer,
	) -> Result<(), SessionError> {
		let msg = message::Group {
			subscribe,
			sequence: group.sequence,
		};

		stream.encode(&msg).await?;

		// TODO abort if the subscription is closed
		let mut size = 0;

		while let Some(chunk) = group.read().await? {
			size += chunk.len();
			stream.write(&chunk).await?;
		}

		// TODO block until all bytes have been acknowledged so we can still reset
		// writer.finish().await?;

		tracing::debug!(size);

		Ok(())
	}

	pub(super) async fn recv_datagrams(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let subscribe = stream.reader.decode().await?;
		self.serve_datagrams(stream, subscribe).await
	}

	#[tracing::instrument("datagrams", skip_all, err, fields(broadcast = subscribe.broadcast, track = subscribe.track, subscribe = subscribe.id))]
	async fn serve_datagrams(
		&mut self,
		stream: &mut Stream,
		subscribe: message::Subscribe,
	) -> Result<(), SessionError> {
		todo!("datagrams");
	}

	pub(super) async fn recv_fetch(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let fetch = stream.reader.decode().await?;
		self.serve_fetch(stream, fetch).await
	}

	#[tracing::instrument("fetch", skip_all, err, fields(broadcast = fetch.broadcast, track = fetch.track, group = fetch.group, offset = fetch.offset))]
	async fn serve_fetch(&mut self, stream: &mut Stream, fetch: message::Fetch) -> Result<(), SessionError> {
		let track = Track::new(&fetch.broadcast, &fetch.track, fetch.priority).build();
		let track = self.subscribe(track).await?;
		let group = track.get(fetch.group)?;

		unimplemented!("TODO fetch");

		/*
		group.skip(fetch.offset);

		while let Some(chunk) = group.read().await {
			let chunk = chunk?;
			writer.write(&chunk).await?;
		}
		*/

		Ok(())
	}

	pub(super) async fn recv_info(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let info = stream.reader.decode().await?;
		self.serve_info(stream, info).await
	}

	#[tracing::instrument("info", skip_all, err, fields(broadcast = info.broadcast, track = info.track))]
	async fn serve_info(&mut self, stream: &mut Stream, info: message::InfoRequest) -> Result<(), SessionError> {
		let track = Track::new(&info.broadcast, &info.track, 0).build();
		let track = self.subscribe(track).await?;

		let info = message::Info {
			group_latest: track.latest(),
			track_priority: track.priority,
			group_expires: track.group_expires,
			group_order: track.group_order,
		};

		stream.writer.encode(&info).await?;

		Ok(())
	}
}
