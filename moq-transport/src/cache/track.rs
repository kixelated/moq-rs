//! A track is a collection of semi-reliable and semi-ordered segments, split into a [Publisher] and [Subscriber] handle.
//!
//! A [Publisher] creates segments with a sequence number and priority.
//! The sequest number is used to determine the order of segments, while the priority is used to determine which segment to transmit first.
//! This may seem counter-intuitive, but is designed for live streaming where the newest segments may be higher priority.
//! A cloned [Publisher] can be used to create segments in parallel, but will error if a duplicate sequence number is used.
//!
//! A [Subscriber] may not receive all segments in order or at all.
//! These segments are meant to be transmitted over congested networks and the key to MoQ Tranport is to not block on them.
//! Segments will be cached for a potentially limited duration added to the unreliable nature.
//! A cloned [Subscriber] will receive a copy of all new segment going forward (fanout).
//!
//! The track is closed with [CacheError::Closed] when all publishers or subscribers are dropped.

use super::{segment, CacheError, Watch};
use std::{fmt, ops::Deref, sync::Arc};

/// Create a track with the given name.
pub fn new(name: &str) -> (Publisher, Subscriber) {
	let state = Watch::new(State::default());
	let info = Arc::new(Info { name: name.to_string() });

	let publisher = Publisher::new(state.clone(), info.clone());
	let subscriber = Subscriber::new(state, info);

	(publisher, subscriber)
}

/// Static information about a track.
#[derive(Debug)]
pub struct Info {
	pub name: String,
}

#[derive(Debug)]
struct State {
	current: Option<segment::Subscriber>,
	epoch: usize,

	// Set when the publisher is closed/dropped, or all subscribers are dropped.
	closed: Result<(), CacheError>,
}

impl State {
	pub fn close(&mut self, err: CacheError) -> Result<(), CacheError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}

	pub fn insert(&mut self, segment: segment::Subscriber) -> Result<(), CacheError> {
		self.closed.clone()?;
		self.current = Some(segment);
		self.epoch += 1;
		Ok(())
	}
}

impl Default for State {
	fn default() -> Self {
		Self {
			current: None,
			epoch: 0,
			closed: Ok(()),
		}
	}
}

/// Creates new segments for a track.
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

	/// Insert a new segment.
	pub fn insert_segment(&mut self, segment: segment::Subscriber) -> Result<(), CacheError> {
		self.state.lock_mut().insert(segment)
	}

	/// Create an insert a segment with the given info.
	pub fn create_segment(&mut self, info: segment::Info) -> Result<segment::Publisher, CacheError> {
		let (publisher, subscriber) = segment::new(info);
		self.insert_segment(subscriber)?;
		Ok(publisher)
	}

	/// Close the segment with an error.
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

/// Receives new segments for a track.
#[derive(Clone)]
pub struct Subscriber {
	state: Watch<State>,
	info: Arc<Info>,
	epoch: usize,

	// Dropped when all subscribers are dropped.
	_dropped: Arc<Dropped>,
}

impl Subscriber {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self {
			state,
			info,
			epoch: 0,
			_dropped,
		}
	}

	/// Block until the next segment arrives
	pub async fn segment(&mut self) -> Result<Option<segment::Subscriber>, CacheError> {
		loop {
			let notify = {
				let state = self.state.lock();

				if self.epoch != state.epoch {
					let segment = state.current.as_ref().unwrap().clone();
					self.epoch = state.epoch;
					return Ok(Some(segment));
				}

				// Otherwise check if we need to return an error.
				match &state.closed {
					Err(CacheError::Closed) => return Ok(None),
					Err(err) => return Err(err.clone()),
					Ok(()) => state.changed(),
				}
			};

			notify.await
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
			.field("epoch", &self.epoch)
			.finish()
	}
}

// Closes the track on Drop.
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
