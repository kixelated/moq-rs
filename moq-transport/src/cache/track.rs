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

use std::{collections::BinaryHeap, fmt, ops::Deref, sync::Arc, time};

use indexmap::IndexMap;

use super::{segment, CacheError, Watch};
use crate::VarInt;

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

struct State {
	// Store segments in received order so subscribers can detect changes.
	// The key is the segment sequence, which could have gaps.
	// A None value means the segment has expired.
	lookup: IndexMap<VarInt, Option<segment::Subscriber>>,

	// Store when segments will expire in a priority queue.
	expires: BinaryHeap<SegmentExpiration>,

	// The number of None entries removed from the start of the lookup.
	pruned: usize,

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

		let entry = match self.lookup.entry(segment.sequence) {
			indexmap::map::Entry::Occupied(_entry) => return Err(CacheError::Duplicate),
			indexmap::map::Entry::Vacant(entry) => entry,
		};

		if let Some(expires) = segment.expires {
			self.expires.push(SegmentExpiration {
				sequence: segment.sequence,
				expires: time::Instant::now() + expires,
			});
		}

		entry.insert(Some(segment));

		// Expire any existing segments on insert.
		// This means if you don't insert then you won't expire... but it's probably fine since the cache won't grow.
		// TODO Use a timer to expire segments at the correct time instead
		self.expire();

		Ok(())
	}

	// Try expiring any segments
	pub fn expire(&mut self) {
		let now = time::Instant::now();
		while let Some(segment) = self.expires.peek() {
			if segment.expires > now {
				break;
			}

			// Update the entry to None while preserving the index.
			match self.lookup.entry(segment.sequence) {
				indexmap::map::Entry::Occupied(mut entry) => entry.insert(None),
				indexmap::map::Entry::Vacant(_) => panic!("expired segment not found"),
			};

			self.expires.pop();
		}

		// Remove None entries from the start of the lookup.
		while let Some((_, None)) = self.lookup.get_index(0) {
			self.lookup.shift_remove_index(0);
			self.pruned += 1;
		}
	}
}

impl Default for State {
	fn default() -> Self {
		Self {
			lookup: Default::default(),
			expires: Default::default(),
			pruned: 0,
			closed: Ok(()),
		}
	}
}

impl fmt::Debug for State {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("State")
			.field("lookup", &self.lookup)
			.field("pruned", &self.pruned)
			.field("closed", &self.closed)
			.finish()
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

	// The index of the next segment to return.
	index: usize,

	// If there are multiple segments to return, we put them in here to return them in priority order.
	pending: BinaryHeap<SegmentPriority>,

	// Dropped when all subscribers are dropped.
	_dropped: Arc<Dropped>,
}

impl Subscriber {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self {
			state,
			info,
			index: 0,
			pending: Default::default(),
			_dropped,
		}
	}

	/// Block until the next segment arrives
	pub async fn segment(&mut self) -> Result<Option<segment::Subscriber>, CacheError> {
		loop {
			let notify = {
				let state = self.state.lock();

				// Get our adjusted index, which could be negative if we've removed more broadcasts than read.
				let mut index = self.index.saturating_sub(state.pruned);

				// Push all new segments into a priority queue.
				while index < state.lookup.len() {
					let (_, segment) = state.lookup.get_index(index).unwrap();

					// Skip None values (expired segments).
					// TODO These might actually be expired, so we should check the expiration time.
					if let Some(segment) = segment {
						self.pending.push(SegmentPriority(segment.clone()));
					}

					index += 1;
				}

				self.index = state.pruned + index;

				// Return the higher priority segment.
				if let Some(segment) = self.pending.pop() {
					return Ok(Some(segment.0));
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
			.field("index", &self.index)
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

// Used to order segments by expiration time.
struct SegmentExpiration {
	sequence: VarInt,
	expires: time::Instant,
}

impl Ord for SegmentExpiration {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		// Reverse order so the earliest expiration is at the top of the heap.
		other.expires.cmp(&self.expires)
	}
}

impl PartialOrd for SegmentExpiration {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl PartialEq for SegmentExpiration {
	fn eq(&self, other: &Self) -> bool {
		self.expires == other.expires
	}
}

impl Eq for SegmentExpiration {}

// Used to order segments by priority
#[derive(Clone)]
struct SegmentPriority(pub segment::Subscriber);

impl Ord for SegmentPriority {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		// Reverse order so the highest priority is at the top of the heap.
		// TODO I let CodePilot generate this code so yolo
		other.0.priority.cmp(&self.0.priority)
	}
}

impl PartialOrd for SegmentPriority {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl PartialEq for SegmentPriority {
	fn eq(&self, other: &Self) -> bool {
		self.0.priority == other.0.priority
	}
}

impl Eq for SegmentPriority {}
