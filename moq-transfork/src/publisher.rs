use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	coding::Stream, message, setup, util::Queue, Broadcast, BroadcastReader, BroadcastWriter, ServeError, Track,
	TrackReader, Unknown, UnknownReader, UnknownWriter,
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
	pub fn announce(&mut self, broadcast: BroadcastReader) -> Result<(), SessionError> {
		match self.broadcasts.lock().unwrap().entry(broadcast.name.clone()) {
			hash_map::Entry::Occupied(_) => return Err(ServeError::Duplicate.into()),
			hash_map::Entry::Vacant(entry) => entry.insert(broadcast.clone()),
		};

		let msg = message::Announce {
			broadcast: broadcast.name.clone(),
		};

		let announce = Announce::new(msg, broadcast);
		if let Err(_) = self.announced.push(announce) {
			return Err(SessionError::Internal);
		}

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
					let this = self.clone();

					tasks.push(async move {
						let res = announce.run(this.webtransport).await;
						this.broadcasts.lock().unwrap().remove(&announce.broadcast);

						res
					});
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?,
			}
		}
	}

	async fn subscribe(&mut self, track: Track) -> Option<TrackReader> {
		let broadcast = self.broadcasts.lock().unwrap().get(&track.broadcast).cloned();
		if let Some(mut broadcast) = broadcast {
			return broadcast.subscribe(track).await;
		}

		if let Some(unknown) = self.unknown.as_mut() {
			return unknown.subscribe(track).await;
		}

		None
	}

	pub(super) async fn run_subscribe(&mut self, mut control: Stream) -> Result<(), SessionError> {
		let subscribe: message::Subscribe = control.reader.decode().await?;

		let track = Track::new(&subscribe.broadcast, &subscribe.track).build();
		let mut track = self.subscribe(track).await.ok_or(ServeError::NotFound)?;

		// TODO this is wrong in the requested case
		let info = message::Info {
			latest: track.latest(),
			group_expires: track.group_expires,
			group_order: track.order.map(Into::into),
			priority: track.priority,
		};
		control.writer.encode(&info).await?;

		// Change to our subscribe order and priority before we start reading.
		track.order = subscribe.group_order.clone().map(Into::into);
		track.priority = Some(subscribe.priority);

		let subscribed = Subscribed::new(self.webtransport.clone(), subscribe, track);
		subscribed.run(control).await
	}

	pub(super) async fn run_datagrams(&mut self, mut control: Stream) -> Result<(), SessionError> {
		let subscribe: message::Subscribe = control.reader.decode().await?;

		todo!("datagrams");
	}

	// TODO close Writer on error
	pub(super) async fn run_fetch(&mut self, mut control: Stream) -> Result<(), SessionError> {
		let fetch: message::Fetch = control.reader.decode().await?;

		let track = Track::new(&fetch.broadcast, &fetch.track).build();
		let track = self.subscribe(track).await.ok_or(ServeError::NotFound)?;
		let group = track.get(fetch.group).ok_or(ServeError::NotFound)?;

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

	pub(super) async fn run_info(&mut self, mut control: Stream) -> Result<(), SessionError> {
		let info: message::InfoRequest = control.reader.decode().await?;

		let track = Track::new(&info.broadcast, &info.track).build();
		let track = self.subscribe(track).await.ok_or(ServeError::NotFound)?;

		let info = message::Info {
			latest: track.latest(),
			priority: track.priority,
			group_expires: track.group_expires,
			group_order: track.order.map(Into::into),
		};
		control.writer.encode(&info).await?;

		Ok(())
	}
}
