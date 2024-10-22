use anyhow::Context;
use bytes::{Bytes, BytesMut};
use coding::{Decode, Encode};
use std::collections::{HashMap, HashSet, VecDeque};
use tokio::sync::oneshot;

use moq_transfork::*;

pub struct ListingProducer {
	track: TrackProducer,
	group: GroupProducer,
	current: HashSet<Path>,
}

impl ListingProducer {
	pub fn new(mut track: TrackProducer) -> Self {
		let mut group = track.append_group();
		group.write_frame(Bytes::new());

		Self {
			track,
			group,
			current: HashSet::new(),
		}
	}

	pub fn insert(&mut self, path: Path) -> bool {
		if !self.current.insert(path.clone()) {
			return false;
		}

		// Create a delta if the current group is small enough.
		if self.current.len() < 2 * self.group.frame_count() {
			self.delta(message::Announce {
				status: message::AnnounceStatus::Active,
				suffix: path,
			});
		} else {
			// Otherwise create a snapshot with every element.
			self.snapshot();
		}

		true
	}

	pub fn remove(&mut self, path: &Path) {
		if !self.current.remove(path) {
			return;
		}

		// Create a delta if the current group is small enough.
		if self.current.len() < 2 * self.group.frame_count() {
			self.delta(message::Announce {
				status: message::AnnounceStatus::Ended,
				suffix: path.clone(),
			});
		} else {
			self.snapshot();
		}
	}

	fn snapshot(&mut self) {
		self.group = self.track.append_group();

		let mut buffer = BytesMut::new();
		for name in &self.current {
			let msg = message::Announce {
				status: message::AnnounceStatus::Active,
				suffix: name.clone(),
			};

			msg.encode(&mut buffer);
		}

		self.group.write_frame(buffer.freeze());
	}

	fn delta(&mut self, msg: message::Announce) {
		let mut buffer = BytesMut::new();
		msg.encode(&mut buffer);
		self.group.write_frame(buffer.freeze());
	}

	pub fn len(&self) -> usize {
		self.current.len()
	}

	pub fn is_empty(&self) -> bool {
		self.current.is_empty()
	}
}

pub struct ListingDrop {}

impl Drop for ListingDrop {
	fn drop(&mut self) {}
}

pub struct ListingConsumer {
	track: TrackConsumer,

	// Keep track of the current group.
	group: Option<GroupConsumer>,

	// Active listings, along with a channel to signal when they are closed.
	active: HashMap<Path, oneshot::Sender<()>>,

	// A list of listings we need to return
	pending: VecDeque<Listing>,
}

impl ListingConsumer {
	pub fn new(track: TrackConsumer) -> Self {
		Self {
			track,
			group: None,

			active: HashMap::new(),
			pending: VecDeque::new(),
		}
	}

	pub async fn next(&mut self) -> anyhow::Result<Option<Listing>> {
		loop {
			if self.group.is_none() && !self.snapshot().await? {
				return Ok(None);
			}

			if let Some(listing) = self.pending.pop_front() {
				return Ok(Some(listing));
			}

			if let Some(listing) = self.delta().await? {
				return Ok(Some(listing));
			}
		}
	}

	// Returns true if a new group was loaded.
	async fn snapshot(&mut self) -> anyhow::Result<bool> {
		let mut group = match self.track.next_group().await? {
			Some(group) => group,
			None => return Ok(false),
		};

		let mut snapshot = group.read_frame().await?.context("missing snapshot")?;
		let mut active = HashMap::new();

		while !snapshot.is_empty() {
			let announce = message::Announce::decode(&mut snapshot)?;
			assert!(matches!(announce.status, message::AnnounceStatus::Active));
			let path = announce.suffix;

			match self.active.remove(&path) {
				Some(tx) => {
					// Existing listing
					active.insert(path, tx);
				}
				None => {
					// New listing
					let (tx, rx) = oneshot::channel();
					active.insert(path.clone(), tx);
					self.pending.push_back(Listing { path, closed: rx });
				}
			}
		}

		// NOTE: This will drop any remaining listings that are only in the old map.
		self.active = active;
		self.group = Some(group);

		Ok(true)
	}

	async fn delta(&mut self) -> anyhow::Result<Option<Listing>> {
		let group = self.group.as_mut().unwrap();

		while let Some(mut payload) = group.read_frame().await? {
			let msg = message::Announce::decode(&mut payload)?;
			let path = msg.suffix;

			match msg.status {
				message::AnnounceStatus::Active => {
					// New listing
					let (tx, rx) = oneshot::channel();
					if self.active.insert(path.clone(), tx).is_some() {
						anyhow::bail!("duplicate listing");
					}
					return Ok(Some(Listing { path, closed: rx }));
				}
				message::AnnounceStatus::Ended => {
					// Removed listing
					self.active.remove(&path).context("non-existent listing")?;
				}
			}
		}

		Ok(None)
	}

	// If you just want to proxy the track
	pub fn into_inner(self) -> TrackConsumer {
		self.track
	}
}

pub struct Listing {
	pub path: Path,
	closed: oneshot::Receiver<()>,
}

impl Listing {
	pub async fn closed(self) -> anyhow::Result<()> {
		self.closed.await.ok();
		Ok(())
	}
}
