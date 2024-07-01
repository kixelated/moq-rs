use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	coding::{Stream, Writer},
	message, setup,
	util::Queue,
	BroadcastReader, Closed, GroupReader, Track, TrackReader, Unknown, UnknownReader, UnknownWriter,
};

use super::{Announce, Session, SessionError};

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
	#[tracing::instrument("session", skip_all, fields(session = self.session.id))]
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

	pub(super) async fn recv_subscribe(&mut self, mut stream: Stream) {
		if let Err(err) = self.recv_subscribe_inner(&mut stream).await {
			stream.writer.reset(err.code());
		}
	}

	#[tracing::instrument("subscribed", skip_all, err, fields(stream = stream.id))]
	async fn recv_subscribe_inner(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let req = stream.reader.decode::<message::Subscribe>().await?;
		tracing::info!(?req);

		let subscribe = req.id;

		let track = Track::new(&req.broadcast, &req.track, req.priority).build();
		let mut track = self.subscribe(track).await?;

		let info = message::Info {
			group_latest: track.latest(),
			group_expires: track.group_expires,
			group_order: track.group_order,
			track_priority: track.priority,
		};

		tracing::info!(?info);
		stream.writer.encode(&info).await?;

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

					let mut this = self.clone();

					tasks.push(async move {
						let res = this.serve_group(subscribe, &mut group).await;
						(group, res)
					});
				},
				res = stream.reader.decode_maybe::<message::SubscribeUpdate>() => {
					match res? {
						Some(update) => {
							// TODO use it
							tracing::info!(?update, "received update");
						},
						None => return Ok(()),
					}
				},
				res = tasks.next(), if !tasks.is_empty() => {
					let (group, res) = res.unwrap();

					if let Err(err) = res {
						let msg = message::GroupDrop {
							sequence: group.sequence,
							count: 0,
							code: err.code(),
						};
						stream.writer.encode(&msg).await?;
					}
				},
			}
		}
	}

	pub async fn serve_group(&mut self, subscribe: u64, group: &mut GroupReader) -> Result<(), SessionError> {
		let stream = self.session.open_uni().await?;
		let mut stream = Writer::new(stream);

		let res = self.serve_group_inner(subscribe, group, &mut stream).await;
		if let Err(err) = &res {
			stream.reset(err.code());
		}

		res
	}

	#[tracing::instrument("group", skip_all, err, fields(subscribe, group = group.sequence, stream = stream.id))]
	pub async fn serve_group_inner(
		&mut self,
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

		while let Some(chunk) = group.read().await? {
			stream.write(&chunk).await?;
		}

		// TODO block until all bytes have been acknowledged so we can still reset
		// writer.finish().await?;

		Ok(())
	}

	pub(super) async fn recv_datagrams(&mut self, mut stream: Stream) {
		if let Err(err) = self.recv_datagrams_inner(&mut stream).await {
			stream.writer.reset(err.code());
		}
	}

	#[tracing::instrument("datagrams", skip_all, err, fields(stream = stream.id))]
	async fn recv_datagrams_inner(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let req: message::Subscribe = stream.reader.decode().await?;
		tracing::info!(?req);

		todo!("datagrams");
	}

	pub(super) async fn recv_fetch(&mut self, mut stream: Stream) {
		if let Err(err) = self.recv_fetch_inner(&mut stream).await {
			stream.writer.reset(err.code());
		}
	}

	#[tracing::instrument("fetch", skip_all, err, fields(stream = stream.id))]
	async fn recv_fetch_inner(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let req: message::Fetch = stream.reader.decode().await?;
		tracing::info!(?req);

		let track = Track::new(&req.broadcast, &req.track, req.priority).build();
		let track = self.subscribe(track).await?;
		let group = track.get(req.group)?;

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

	pub(super) async fn recv_info(&mut self, mut stream: Stream) {
		if let Err(err) = self.recv_info_inner(&mut stream).await {
			stream.writer.reset(err.code());
		}
	}

	#[tracing::instrument("info", skip_all, err, fields(stream = stream.id))]
	async fn recv_info_inner(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let request: message::InfoRequest = stream.reader.decode().await?;
		tracing::info!(?request);

		let track = Track::new(&request.broadcast, &request.track, 0).build();
		let track = self.subscribe(track).await?;

		let response = message::Info {
			group_latest: track.latest(),
			track_priority: track.priority,
			group_expires: track.group_expires,
			group_order: track.group_order,
		};
		tracing::info!(?response);

		stream.writer.encode(&response).await?;

		Ok(())
	}
}
