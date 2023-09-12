use std::{
	collections::{hash_map, HashMap, VecDeque},
	ops::Deref,
	sync::Arc,
};

use crate::Error;

use super::{track, Watch};

/// Creates a new broadcast by name, returning three handles.
///   - Publisher:  Used to create tracks by name.
///   - Subscriber: Used to request tracks by name.
///   - Unknown:    Used to optionally create tracks on request.
///
/// When all Publishers are dropped:
///   - Nothing happens... but you can't create new tracks.
///
/// When Unknown is dropped:
///   - Any unknown tracks will return a Error::NotFound.
///
/// When all Publishers AND Unknown are dropped:
///   - broadcast::Subscriber::get() will return Error::Closed, even if the track exists.
///
/// When all Subscribers are dropped:
///   - Publisher::track() returns Error::Closed.
///   - Unknown::track() returns None.
pub fn new(name: &str) -> (Publisher, Subscriber, Unknown) {
	let state = Watch::new(State::default());
	let info = Arc::new(Info { name: name.to_string() });
	let dropped = Arc::new(Dropped::new(state.clone()));

	let publisher = Publisher::new(state.clone(), info.clone(), dropped.clone());
	let subscriber = Subscriber::new(state.clone(), info.clone());
	let unknown = Unknown::new(state, info, dropped);

	(publisher, subscriber, unknown)
}

#[derive(Debug)]
pub struct Info {
	pub name: String,
}

#[derive(Debug)]
struct State {
	tracks: HashMap<String, track::Subscriber>,
	requested: Option<VecDeque<track::Publisher>>, // Set to None when Unknown is dropped.
	closed: Result<(), Error>,
}

impl State {
	pub fn get(&self, name: &str) -> Result<Option<track::Subscriber>, Error> {
		self.closed?;
		Ok(self.tracks.get(name).cloned())
	}

	pub fn insert(&mut self, track: track::Subscriber) -> Result<(), Error> {
		self.closed?;

		match self.tracks.entry(track.name.clone()) {
			hash_map::Entry::Occupied(_) => return Err(Error::Duplicate),
			hash_map::Entry::Vacant(v) => v.insert(track),
		};

		Ok(())
	}

	pub fn request(&mut self, name: &str) -> Result<track::Subscriber, Error> {
		self.closed?;

		// Make sure Unknown hasn't been dropped yet.
		let requested = self.requested.as_mut().ok_or(Error::NotFound)?;

		// Create a new track.
		let (publisher, subscriber) = track::new(name);

		// Insert the track into our Map so we deduplicate future requests.
		self.tracks.insert(name.to_string(), subscriber.clone());

		// Send the track to Unknown to handle.
		requested.push_back(publisher);

		Ok(subscriber)
	}

	pub fn has_next(&self) -> Result<bool, Error> {
		// Check if there's any elements in the queue before checking closed.
		if self.requested.as_ref().filter(|q| !q.is_empty()).is_some() {
			return Ok(true);
		}

		self.closed?;
		Ok(false)
	}

	pub fn next(&mut self) -> track::Publisher {
		// We panic instead of erroring to avoid a nasty wakeup loop if you don't call has_next first.
		self.requested
			.as_mut()
			.expect("queue closed")
			.pop_front()
			.expect("no entry in queue")
	}

	pub fn close(&mut self, err: Error) -> Result<(), Error> {
		self.closed?;
		self.closed = Err(err);
		Ok(())
	}
}

impl Default for State {
	fn default() -> Self {
		Self {
			tracks: HashMap::new(),
			closed: Ok(()),
			requested: Some(VecDeque::new()),
		}
	}
}

#[derive(Debug, Clone)]
pub struct Publisher {
	state: Watch<State>,
	info: Arc<Info>,

	_dropped: Arc<Dropped>,
}

impl Publisher {
	fn new(state: Watch<State>, info: Arc<Info>, _dropped: Arc<Dropped>) -> Self {
		Self { state, info, _dropped }
	}

	/// Create a new track with the given name, inserting it into the broadcast.
	pub fn create_track(&mut self, name: &str) -> Result<track::Publisher, Error> {
		let (publisher, subscriber) = track::new(name);
		self.state.lock_mut().insert(subscriber)?;
		Ok(publisher)
	}

	/// Insert a track into the broadcast.
	pub fn insert_track(&mut self, track: track::Subscriber) -> Result<(), Error> {
		self.state.lock_mut().insert(track)
	}

	pub fn close(self, err: Error) -> Result<(), Error> {
		self.state.lock_mut().close(err)
	}
}

impl Deref for Publisher {
	type Target = Info;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

#[derive(Clone, Debug)]
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
	pub fn get_track(&self, name: &str) -> Result<track::Subscriber, Error> {
		let state = self.state.lock();
		if let Some(track) = state.get(name)? {
			return Ok(track);
		}

		// Request a new track if it does not exist.
		state.as_mut().request(name)
	}
}

impl Deref for Subscriber {
	type Target = Info;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

// A handle that closes the broadcast when dropped:
// - when all Subscribers are dropped or
// - when Publisher and Unknown are dropped.
#[derive(Debug)]
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
		self.state.lock_mut().close(Error::Closed).ok();
	}
}

/// Contains a queue of requested tracks that do not exist.
/// The publisher may wish to handle these tracks, or it can drop the Unknown handle to automatically close them with a Not Found error.
pub struct Unknown {
	state: Watch<State>,
	info: Arc<Info>,

	_dropped: Arc<Dropped>,
}

impl Unknown {
	fn new(state: Watch<State>, info: Arc<Info>, _dropped: Arc<Dropped>) -> Self {
		Self { state, info, _dropped }
	}

	pub async fn next_track(&mut self) -> Result<Option<track::Publisher>, Error> {
		loop {
			let notify = {
				let state = self.state.lock();
				if state.has_next()? {
					return Ok(Some(state.as_mut().next()));
				}

				state.changed()
			};

			notify.await;
		}
	}
}

impl Deref for Unknown {
	type Target = Info;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

impl Drop for Unknown {
	fn drop(&mut self) {
		// Close any entries in the queue that we didn't read yet.
		let mut state = self.state.lock_mut();
		while let Ok(true) = state.has_next() {
			state.next().close(Error::NotFound).ok();
		}

		// Prevent new requests.
		state.requested = None;
	}
}
