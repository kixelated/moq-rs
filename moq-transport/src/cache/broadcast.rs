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
//! The broadcast is automatically closed with [CacheError::Closed] when [Publisher] is dropped, or all [Subscriber]s are dropped.
use std::{
	collections::{hash_map, HashMap, VecDeque},
	fmt,
	ops::Deref,
	sync::Arc,
};

use super::{track, CacheError, Watch};

/// Create a new broadcast.
pub fn new(id: &str) -> (Publisher, Subscriber) {
	let state = Watch::new(State::default());
	let info = Arc::new(Info { id: id.to_string() });

	let publisher = Publisher::new(state.clone(), info.clone());
	let subscriber = Subscriber::new(state, info);

	(publisher, subscriber)
}

/// Static information about a broadcast.
#[derive(Debug)]
pub struct Info {
	pub id: String,
}

/// Dynamic information about the broadcast.
#[derive(Debug)]
struct State {
	tracks: HashMap<String, track::Subscriber>,
	requested: VecDeque<track::Publisher>,
	closed: Result<(), CacheError>,
}

impl State {
	pub fn get(&self, name: &str) -> Result<Option<track::Subscriber>, CacheError> {
		// Don't check closed, so we can return from cache.
		Ok(self.tracks.get(name).cloned())
	}

	pub fn insert(&mut self, track: track::Subscriber) -> Result<(), CacheError> {
		self.closed.clone()?;

		match self.tracks.entry(track.name.clone()) {
			hash_map::Entry::Occupied(_) => return Err(CacheError::Duplicate),
			hash_map::Entry::Vacant(v) => v.insert(track),
		};

		Ok(())
	}

	pub fn request(&mut self, name: &str) -> Result<track::Subscriber, CacheError> {
		self.closed.clone()?;

		// Create a new track.
		let (publisher, subscriber) = track::new(name);

		// Insert the track into our Map so we deduplicate future requests.
		self.tracks.insert(name.to_string(), subscriber.clone());

		// Send the track to the Publisher to handle.
		self.requested.push_back(publisher);

		Ok(subscriber)
	}

	pub fn has_next(&self) -> Result<bool, CacheError> {
		// Check if there's any elements in the queue before checking closed.
		if !self.requested.is_empty() {
			return Ok(true);
		}

		self.closed.clone()?;
		Ok(false)
	}

	pub fn next(&mut self) -> track::Publisher {
		// We panic instead of erroring to avoid a nasty wakeup loop if you don't call has_next first.
		self.requested.pop_front().expect("no entry in queue")
	}

	pub fn close(&mut self, err: CacheError) -> Result<(), CacheError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}
}

impl Default for State {
	fn default() -> Self {
		Self {
			tracks: HashMap::new(),
			closed: Ok(()),
			requested: VecDeque::new(),
		}
	}
}

/// Publish new tracks for a broadcast by name.
// TODO remove Clone
#[derive(Clone)]
pub struct Publisher {
	state: Watch<State>,
	info: Arc<Info>,
	_dropped: Arc<Dropped>,
}

impl Publisher {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self { state, info, _dropped }
	}

	/// Create a new track with the given name, inserting it into the broadcast.
	pub fn create_track(&mut self, name: &str) -> Result<track::Publisher, CacheError> {
		let (publisher, subscriber) = track::new(name);
		self.state.lock_mut().insert(subscriber)?;
		Ok(publisher)
	}

	/// Insert a track into the broadcast.
	pub fn insert_track(&mut self, track: track::Subscriber) -> Result<(), CacheError> {
		self.state.lock_mut().insert(track)
	}

	/// Block until the next track requested by a subscriber.
	pub async fn next_track(&mut self) -> Result<track::Publisher, CacheError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if state.has_next()? {
					return Ok(state.into_mut().next());
				}

				state.changed()
			};

			notify.await;
		}
	}

	/// Close the broadcast with an error.
	pub fn close(self, err: CacheError) -> Result<(), CacheError> {
		self.state.lock_mut().close(err)
	}
}

impl Deref for Publisher {
	type Target = Info;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

impl fmt::Debug for Publisher {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Publisher")
			.field("state", &self.state)
			.field("info", &self.info)
			.finish()
	}
}

/// Subscribe to a broadcast by requesting tracks.
///
/// This can be cloned to create handles.
#[derive(Clone)]
pub struct Subscriber {
	state: Watch<State>,
	info: Arc<Info>,
	_dropped: Arc<Dropped>,
}

impl Subscriber {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self { state, info, _dropped }
	}

	/// Get a track from the broadcast by name.
	/// If the track does not exist, it will be created and potentially fufilled by the publisher (via Unknown).
	/// Otherwise, it will return [CacheError::NotFound].
	pub fn get_track(&self, name: &str) -> Result<track::Subscriber, CacheError> {
		let state = self.state.lock();
		if let Some(track) = state.get(name)? {
			return Ok(track);
		}

		// Request a new track if it does not exist.
		state.into_mut().request(name)
	}

	/// Check if the broadcast is closed, either because the publisher was dropped or called [Publisher::close].
	pub fn is_closed(&self) -> Option<CacheError> {
		self.state.lock().closed.as_ref().err().cloned()
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

impl Deref for Subscriber {
	type Target = Info;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

impl fmt::Debug for Subscriber {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Subscriber")
			.field("state", &self.state)
			.field("info", &self.info)
			.finish()
	}
}

// A handle that closes the broadcast when dropped:
// - when all Subscribers are dropped or
// - when Publisher and Unknown are dropped.
struct Dropped {
	state: Watch<State>,
}

impl Dropped {
	fn new(state: Watch<State>) -> Self {
		Self { state }
	}
}

impl Drop for Dropped {
	fn drop(&mut self) {
		self.state.lock_mut().close(CacheError::Closed).ok();
	}
}
