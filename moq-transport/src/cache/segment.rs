//! A segment is a stream of fragments with a header, split into a [Publisher] and [Subscriber] handle.
//!
//! A [Publisher] writes an ordered stream of fragments.
//! Each fragment can have a sequence number, allowing the subscriber to detect gaps fragments.
//!
//! A [Subscriber] reads an ordered stream of fragments.
//! The subscriber can be cloned, in which case each subscriber receives a copy of each fragment. (fanout)
//!
//! The segment is closed with [CacheError::Closed] when all publishers or subscribers are dropped.
use core::fmt;
use std::{ops::Deref, sync::Arc, time};

use crate::VarInt;

use super::{fragment, CacheError, Watch};

/// Create a new segment with the given info.
pub fn new(info: Info) -> (Publisher, Subscriber) {
	let state = Watch::new(State::default());
	let info = Arc::new(info);

	let publisher = Publisher::new(state.clone(), info.clone());
	let subscriber = Subscriber::new(state, info);

	(publisher, subscriber)
}

/// Static information about the segment.
#[derive(Debug)]
pub struct Info {
	// The sequence number of the segment within the track.
	// NOTE: These may be received out of order or with gaps.
	pub sequence: VarInt,

	// The priority of the segment within the BROADCAST.
	pub priority: u32,

	// Cache the segment for at most this long.
	pub expires: Option<time::Duration>,
}

struct State {
	// The data that has been received thus far.
	fragments: Vec<fragment::Subscriber>,

	// Set when the publisher is dropped.
	closed: Result<(), CacheError>,
}

impl State {
	pub fn close(&mut self, err: CacheError) -> Result<(), CacheError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}
}

impl Default for State {
	fn default() -> Self {
		Self {
			fragments: Vec::new(),
			closed: Ok(()),
		}
	}
}

impl fmt::Debug for State {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("State")
			.field("fragments", &self.fragments)
			.field("closed", &self.closed)
			.finish()
	}
}

/// Used to write data to a segment and notify subscribers.
pub struct Publisher {
	// Mutable segment state.
	state: Watch<State>,

	// Immutable segment state.
	info: Arc<Info>,

	// Closes the segment when all Publishers are dropped.
	_dropped: Arc<Dropped>,
}

impl Publisher {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self { state, info, _dropped }
	}

	// Not public because it's a footgun.
	pub(crate) fn push_fragment(
		&mut self,
		sequence: VarInt,
		size: Option<usize>,
	) -> Result<fragment::Publisher, CacheError> {
		let (publisher, subscriber) = fragment::new(fragment::Info { sequence, size });

		let mut state = self.state.lock_mut();
		state.closed.clone()?;
		state.fragments.push(subscriber);
		Ok(publisher)
	}

	/// Write a fragment
	pub fn fragment(&mut self, sequence: VarInt, size: usize) -> Result<fragment::Publisher, CacheError> {
		self.push_fragment(sequence, Some(size))
	}

	/// Write the last fragment, which means size can be unknown.
	pub fn final_fragment(mut self, sequence: VarInt) -> Result<fragment::Publisher, CacheError> {
		self.push_fragment(sequence, None)
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

/// Notified when a segment has new data available.
#[derive(Clone)]
pub struct Subscriber {
	// Modify the segment state.
	state: Watch<State>,

	// Immutable segment state.
	info: Arc<Info>,

	// The number of chunks that we've read.
	// NOTE: Cloned subscribers inherit this index, but then run in parallel.
	index: usize,

	// Dropped when all Subscribers are dropped.
	_dropped: Arc<Dropped>,
}

impl Subscriber {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));

		Self {
			state,
			info,
			index: 0,
			_dropped,
		}
	}

	/// Block until the next chunk of bytes is available.
	pub async fn fragment(&mut self) -> Result<Option<fragment::Subscriber>, CacheError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if self.index < state.fragments.len() {
					let fragment = state.fragments[self.index].clone();
					self.index += 1;
					return Ok(Some(fragment));
				}

				match &state.closed {
					Err(CacheError::Closed) => return Ok(None),
					Err(err) => return Err(err.clone()),
					Ok(()) => state.changed(),
				}
			};

			notify.await; // Try again when the state changes
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
			.field("index", &self.index)
			.finish()
	}
}

struct Dropped {
	// Modify the segment state.
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
