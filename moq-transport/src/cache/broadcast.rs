//! A broadcast is a collection of tracks, split into two handles: [Publisher] and [Subscriber].
//!
//! The [Publisher] can create tracks, either manually or on request.
//! It receives all requests by a [Subscriber] for a tracks that don't exist.
//! The simplest implementation is to close every unknown track with [CacheError::NotFound].
//!
//! A [Subscriber] can request tracks by name.
//! If the track already exists, it will be returned.
//! If the track doesn't exist, it will be sent to [Unknown] to be handled.
//! A [Subscriber] can be cloned to create multiple subscriptions.
//!
//! The broadcast is automatically closed with [CacheError::Done] when [Publisher] is dropped, or all [Subscriber]s are dropped.
use std::{
	collections::{hash_map, HashMap},
	ops::Deref,
	sync::Arc,
};

use super::{Track, TrackPublisher, TrackSubscriber};
use crate::{error::CacheError, publisher, util::Watch, Publisher, ServeError, SubscribeError};
use futures::{
	stream::{FuturesUnordered, StreamExt},
	FutureExt,
};

/// Static information about a broadcast.
#[derive(Debug)]
pub struct Broadcast {
	pub namespace: String,
}

/// Dynamic information about the broadcast.
struct BroadcastState {
	tracks: HashMap<String, TrackSubscriber>,
	closed: Result<(), CacheError>,
}

impl BroadcastState {
	pub fn get(&self, name: &str) -> Result<Option<TrackSubscriber>, CacheError> {
		match self.tracks.get(name) {
			Some(track) => Ok(Some(track.clone())),
			None => self.closed.clone().map(|_| None),
		}
	}

	pub fn insert(&mut self, track: TrackSubscriber) -> Result<(), CacheError> {
		match self.tracks.entry(track.name.clone()) {
			hash_map::Entry::Occupied(_) => return Err(CacheError::Duplicate),
			hash_map::Entry::Vacant(v) => v.insert(track),
		};

		Ok(())
	}

	pub fn remove(&mut self, name: &str) -> Option<TrackSubscriber> {
		self.tracks.remove(name)
	}

	pub fn close(&mut self, err: CacheError) -> Result<(), CacheError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}
}

impl Default for BroadcastState {
	fn default() -> Self {
		Self {
			tracks: HashMap::new(),
			closed: Ok(()),
		}
	}
}

/// Publish new tracks for a broadcast by name.
pub struct BroadcastPublisher {
	state: Watch<BroadcastState>,
	info: Arc<Broadcast>,
	subscriber: BroadcastSubscriber,
}

impl BroadcastPublisher {
	pub fn new(name: &str) -> Self {
		let state = Watch::new(BroadcastState::default());
		let info = Arc::new(Broadcast {
			namespace: name.to_owned(),
		});
		let subscriber = BroadcastSubscriber::new(state.clone(), info.clone());

		Self {
			state,
			info,
			subscriber,
		}
	}

	/// Create a new track with the given name, inserting it into the broadcast.
	pub fn create_track(&mut self, track: &str) -> Result<TrackPublisher, CacheError> {
		let publisher = TrackPublisher::new(Track {
			namespace: self.namespace.clone(),
			name: track.to_owned(),
		});

		self.state.lock_mut().insert(publisher.subscribe())?;
		Ok(publisher)
	}

	pub fn remove_track(&mut self, track: &str) -> Option<TrackSubscriber> {
		self.state.lock_mut().remove(track)
	}

	pub fn subscribe(&self) -> BroadcastSubscriber {
		self.subscriber.clone()
	}

	/// Close the broadcast with an error.
	pub fn close(self, err: CacheError) -> Result<(), CacheError> {
		self.state.lock_mut().close(err)
	}
}

impl Deref for BroadcastPublisher {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Subscribe to a broadcast by requesting tracks.
///
/// This can be cloned to create handles.
#[derive(Clone)]
pub struct BroadcastSubscriber {
	state: Watch<BroadcastState>,
	info: Arc<Broadcast>,
}

impl BroadcastSubscriber {
	fn new(state: Watch<BroadcastState>, info: Arc<Broadcast>) -> Self {
		Self { state, info }
	}

	/// Get a track from the broadcast by name.
	pub fn get_track(&self, name: &str) -> Result<Option<TrackSubscriber>, CacheError> {
		self.state.lock().get(name)
	}

	/// Check if the broadcast is closed, either because the publisher was dropped or called [Publisher::close].
	pub fn is_closed(&self) -> Option<CacheError> {
		self.state.lock().closed.as_ref().err().cloned()
	}

	// TODO This is wrong because it assumes we're the only publisher, but Publisher is Clone.
	pub async fn serve(self, mut session: Publisher) -> Result<(), ServeError> {
		// Dropped automatically at the end of the function.
		let _announce = session.announce(self.namespace.clone()).await?;

		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				subscribe = session.subscribed() => tasks.push(self.serve_subscribe(subscribe?).boxed()),
				res = tasks.next(), if !tasks.is_empty() => { res.unwrap().ok(); } // TODO log failures?
			}
		}
	}

	async fn serve_subscribe(&self, subscribe: publisher::SubscribePending) -> Result<(), ServeError> {
		if subscribe.namespace() != self.namespace {
			subscribe.reject(SubscribeError::NotFound.code())?;
			return Ok(());
		}

		let track = match self.get_track(subscribe.name())? {
			Some(track) => track,
			None => {
				subscribe.reject(SubscribeError::NotFound.code())?;
				return Ok(());
			}
		};

		// TODO log these errors?
		let subscribe = subscribe.accept(publisher::SubscribeResponse {
			latest: track.latest(),
			expires: None,
		})?;
		track.serve(subscribe).await?;

		Ok(())
	}

	/// Wait until if the broadcast is closed, either because the publisher was dropped or called [Publisher::close].
	pub async fn closed(&self) -> CacheError {
		loop {
			let notify = {
				let state = self.state.lock();
				if let Some(err) = state.closed.as_ref().err() {
					return err.clone();
				}

				state.changed()
			};

			notify.await;
		}
	}
}

impl Deref for BroadcastSubscriber {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
