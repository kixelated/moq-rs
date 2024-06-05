use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	message,
	serve::{self, ServeError, Track, TrackReader, Unknown, UnknownReader, UnknownWriter},
	setup,
	util::Queue,
};

use super::{Announce, Control, Session, SessionError, Subscribed};

#[derive(Clone)]
pub struct Publisher {
	webtransport: web_transport::Session,

	// Used to route incoming subscriptions
	broadcasts: Arc<Mutex<HashMap<String, serve::BroadcastReader>>>,

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

	/// Announce a broadcast and serve tracks using the provided [serve::BroadcastReader].
	/// The caller uses [serve::BroadcastWriter] to populate static tracks.
	/// Use [Publisher::unknown] to handle unknown tracks, even for announced broadcasts.
	pub fn announce(&mut self, broadcast: serve::BroadcastReader) -> Result<Announce, SessionError> {
		let name = broadcast.name.clone();

		match self.broadcasts.lock().unwrap().entry(name.clone()) {
			hash_map::Entry::Occupied(_) => return Err(ServeError::Duplicate.into()),
			hash_map::Entry::Vacant(entry) => entry.insert(broadcast),
		};

		let msg = message::Announce {
			broadcast: name.clone(),
		};

		let announce = Announce::new(msg);
		if let Err(_) = self.announced.push(announce.split()) {
			return Err(SessionError::Internal);
		}

		Ok(announce)
	}

	/// Creates and returns a handler to deal with unknown tracks.
	/// This can only be called once, otherwise it returns None.
	pub fn unknown(&mut self) -> Option<UnknownWriter> {
		if self.unknown.is_some() {
			return None;
		}

		let (writer, reader) = Unknown::produce();
		self.unknown = Some(reader);

		Some(writer)
	}

	pub(super) async fn run(&mut self) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(announce) = self.announced.pop() => {
					let mut webtransport = self.webtransport.clone();
					tasks.push(async move {
						let control = Control::open(&mut webtransport, message::Control::Announce).await?;
						announce.run(control).await
					});
				},
				res = tasks.next(), if !tasks.is_empty() => {
					match res.unwrap() {
						Ok(broadcast) => self.broadcasts.lock().unwrap().remove(&broadcast),
						Err(err) => return Err(err),
					};
				},
			}
		}
	}

	async fn route(&mut self, track: Track) -> Option<TrackReader> {
		let broadcast = self.broadcasts.lock().unwrap().get(&track.broadcast).cloned();
		if let Some(mut broadcast) = broadcast {
			return broadcast.request(&track.name).await;
		}

		if let Some(unknown) = self.unknown.as_mut() {
			return unknown.request(track).await;
		}

		None
	}

	pub(super) async fn run_subscribe(&mut self, mut control: Control) -> Result<(), SessionError> {
		let subscribe: message::Subscribe = control.reader.decode().await?;

		let track = Track::new(&subscribe.broadcast, &subscribe.track).build();
		let mut track = self.route(track).await.ok_or(ServeError::NotFound)?;

		// TODO this is wrong in the requested case
		let info = message::Info {
			latest: track.latest(),
			default_order: track.order.map(Into::into),
			default_priority: track.priority,
		};
		control.writer.encode(&info).await?;

		// Change to our subscribe order and priority before we start reading.
		track.order = subscribe.order.clone().map(Into::into);
		track.priority = Some(subscribe.priority);

		let subscribed = Subscribed::new(self.webtransport.clone(), subscribe, track);
		subscribed.run(control).await
	}

	pub(super) async fn run_datagrams(&mut self, mut control: Control) -> Result<(), SessionError> {
		let subscribe: message::Subscribe = control.reader.decode().await?;

		todo!("datagrams");
	}

	// TODO close Writer on error
	pub(super) async fn run_fetch(&mut self, mut control: Control) -> Result<(), SessionError> {
		let fetch: message::Fetch = control.reader.decode().await?;

		let track = Track::new(&fetch.broadcast, &fetch.track).build();
		let track = self.route(track).await.ok_or(ServeError::NotFound)?;
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

	pub(super) async fn run_info(&mut self, mut control: Control) -> Result<(), SessionError> {
		let info: message::InfoRequest = control.reader.decode().await?;

		let track = Track::new(&info.broadcast, &info.track).build();
		let track = self.route(track).await.ok_or(ServeError::NotFound)?;

		let info = message::Info {
			latest: track.latest(),
			default_order: track.order.map(Into::into),
			default_priority: track.priority,
		};
		control.writer.encode(&info).await?;

		Ok(())
	}
}
