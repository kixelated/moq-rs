use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	message,
	serve::{self, ServeError},
	setup,
	util::Queue,
};

use super::{Announce, Control, Session, SessionError, Subscribed};

// TODO remove Clone.
#[derive(Clone)]
pub struct Publisher {
	webtransport: web_transport::Session,

	// Used to route incoming subscriptions
	broadcasts: Arc<Mutex<HashMap<String, serve::BroadcastReader>>>,

	announced: Queue<Announce>,
}

impl Publisher {
	pub(crate) fn new(webtransport: web_transport::Session) -> Self {
		Self {
			webtransport,
			broadcasts: Default::default(),
			announced: Default::default(),
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
	/// The caller uses [serve::BroadcastWriter] to populate static tracks and [serve::BroadcastRequest] for dynamic tracks.
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

	pub fn subscribed(&mut self, unknown: serve::BroadcastRequest) -> Result<Subscribed, ServeError> {
		let broadcast = unknown.broadcast.clone();
		let track = self.get_track(&broadcast, &unknown.track)?;

		Ok(Subscribed::new(self.webtransport.clone(), unknown, track))
	}

	fn get_track(&self, broadcast: &str, track: &str) -> Result<serve::TrackReader, ServeError> {
		// Route the subscribe to an announce.
		self.broadcasts
			.lock()
			.unwrap()
			.get_mut(broadcast)
			.ok_or(ServeError::NotFound)?
			.subscribe(track)
			.ok_or(ServeError::NotFound)
	}

	pub(super) async fn run(&mut self) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(announce) = self.announced.pop() => {
					let mut webtransport = self.webtransport.clone();
					tasks.push(async move {
						let control = Control::open(&mut webtransport, message::StreamBi::Announce).await?;
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

	pub(super) async fn run_subscribe(&mut self, mut control: Control) -> Result<(), SessionError> {
		let subscribe: message::Subscribe = control.reader.decode().await?;

		let mut track = self.get_track(&subscribe.broadcast, &subscribe.track)?;

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

		let track = self.get_track(&fetch.broadcast, &fetch.track)?;

		let mut group = track.get_group(fetch.group).ok_or(ServeError::NotFound)?;

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

		let track = self.get_track(&info.broadcast, &info.track)?;

		let info = message::Info {
			latest: track.latest(),
			default_order: track.order.map(Into::into),
			default_priority: track.priority,
		};
		control.writer.encode(&info).await?;

		Ok(())
	}
}
