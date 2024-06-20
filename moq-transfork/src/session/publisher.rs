use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	coding::Stream, message, setup, util::Queue, BroadcastReader, Closed, Track, TrackReader, Unknown, UnknownReader,
	UnknownWriter,
};

use super::{Announce, Session, SessionError, Subscribed};

#[derive(Clone)]
pub struct Publisher {
	webtransport: web_transport::Session,

	// Used to route incoming subscriptions
	broadcasts: Arc<Mutex<HashMap<String, BroadcastReader>>>,

	announced: Queue<Announce>,
	unknown: Option<UnknownReader>,
}

impl Publisher {
	pub(crate) fn new(webtransport: web_transport::Session) -> Self {
		Self {
			webtransport,
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
	pub async fn announce(&mut self, broadcast: BroadcastReader) -> Result<(), SessionError> {
		match self.broadcasts.lock().unwrap().entry(broadcast.name.clone()) {
			hash_map::Entry::Occupied(_) => return Err(Closed::Duplicate.into()),
			hash_map::Entry::Vacant(entry) => entry.insert(broadcast.clone()),
		};

		let mut stream = Stream::open(&mut self.webtransport, message::Control::Announce).await?;
		if let Err(err) = self.announce_start(&mut stream, &broadcast).await {
			stream.writer.reset(err.code());
		}

		let announce = Announce::new(broadcast, stream);
		if self.announced.push(announce).is_err() {
			return Err(SessionError::Internal);
		}

		Ok(())
	}

	async fn announce_start(&mut self, stream: &mut Stream, broadcast: &BroadcastReader) -> Result<(), SessionError> {
		log::info!("announcing: broadcast={}", broadcast.name);

		let msg = message::Announce {
			broadcast: broadcast.name.clone(),
		};

		stream.writer.encode(&msg).await?;
		let _ = stream.reader.decode::<message::AnnounceOk>().await;

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
				Some(mut announce) = self.announced.pop() => {
					tasks.push(async move {
						announce.run().await;
						announce
					});
				},
				res = tasks.next(), if !tasks.is_empty() => {
					let announce = res.unwrap();
					self.broadcasts.lock().unwrap().remove(&announce.broadcast);
				},
			}
		}
	}

	async fn subscribe(&mut self, track: Track) -> Result<TrackReader, Closed> {
		let broadcast = self.broadcasts.lock().unwrap().get(&track.broadcast).cloned();
		if let Some(mut broadcast) = broadcast {
			log::info!("found announcement: {:?}", broadcast.info);
			return broadcast.subscribe(track).await;
		}

		if let Some(unknown) = self.unknown.as_mut() {
			log::info!("found unnknown");
			return unknown.subscribe(track).await;
		}

		log::info!("did not find unknown");

		Err(Closed::UnknownBroadcast)
	}

	pub(super) async fn run_subscribe(&mut self, control: &mut Stream) -> Result<(), SessionError> {
		let subscribe: message::Subscribe = control.reader.decode().await?;

		log::info!(
			"received subscription: broadcast={} track={}",
			subscribe.broadcast,
			subscribe.track,
		);

		let track = Track::new(&subscribe.broadcast, &subscribe.track, subscribe.priority).build();
		let track = self.subscribe(track).await?;

		let info = message::Info {
			group_latest: track.latest(),
			group_expires: track.group_expires,
			group_order: track.group_order,
			track_priority: track.priority,
		};
		control.writer.encode(&info).await?;

		let subscribed = Subscribed::new(self.webtransport.clone(), subscribe, track);
		subscribed.run(control).await
	}

	pub(super) async fn run_datagrams(&mut self, control: &mut Stream) -> Result<(), SessionError> {
		let subscribe: message::Subscribe = control.reader.decode().await?;

		log::info!(
			"received datagram subscription: broadcast={} track={}",
			subscribe.broadcast,
			subscribe.track,
		);

		todo!("datagrams");
	}

	// TODO close Writer on error
	pub(super) async fn run_fetch(&mut self, control: &mut Stream) -> Result<(), SessionError> {
		let fetch: message::Fetch = control.reader.decode().await?;

		log::info!("received fetch: broadcast={} track={}", fetch.broadcast, fetch.track,);

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

	pub(super) async fn run_info(&mut self, control: &mut Stream) -> Result<(), SessionError> {
		let info: message::InfoRequest = control.reader.decode().await?;

		log::info!(
			"received info request: broadcast={} track={}",
			info.broadcast,
			info.track,
		);

		let track = Track::new(&info.broadcast, &info.track, 0).build();
		let track = self.subscribe(track).await?;

		let info = message::Info {
			group_latest: track.latest(),
			track_priority: track.priority,
			group_expires: track.group_expires,
			group_order: track.group_order,
		};
		control.writer.encode(&info).await?;

		Ok(())
	}
}
