use anyhow::Context;
use bytes::{Bytes, BytesMut};
use std::collections::{HashMap, HashSet, VecDeque};
use tokio::sync::oneshot;

use moq_transfork::prelude::*;

pub struct ListingProducer {
	track: TrackProducer,
	group: GroupProducer,
	current: HashSet<String>,
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

	pub fn insert(&mut self, name: String) -> anyhow::Result<()> {
		if !self.current.insert(name.clone()) {
			anyhow::bail!("duplicate");
		}

		// Create a delta if the current group is small enough.
		if self.current.len() < 2 * self.group.frame_count() {
			let msg = format!("+{}", name);
			self.group.write_frame(msg.into());
		} else {
			// Otherwise create a snapshot with every element.
			self.snapshot();
		}

		Ok(())
	}

	pub fn remove(&mut self, name: &str) -> anyhow::Result<()> {
		if !self.current.remove(name) {
			anyhow::bail!("missing");
		}

		// Create a delta if the current group is small enough.
		if self.current.len() < 2 * self.group.frame_count() {
			let msg = format!("-{}", name);
			self.group.write_frame(msg.into());
		} else {
			self.snapshot();
		}

		Ok(())
	}

	fn snapshot(&mut self) {
		self.group = self.track.append_group();

		let mut msg = BytesMut::new();
		for name in &self.current {
			msg.extend_from_slice(name.as_bytes());
			msg.extend_from_slice(b"\n");
		}

		self.group.write_frame(msg.freeze());
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
	active: HashMap<String, oneshot::Sender<()>>,

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

		let snapshot = group.read_frame().await?.context("missing snapshot")?;

		if snapshot.is_empty() {
			self.active.drain();
			self.group = Some(group);
			return Ok(true);
		}

		let mut active = HashMap::new();

		for name in snapshot
			.split(|&b| b == b'\n')
			.map(|s| String::from_utf8_lossy(s).to_string())
		{
			match self.active.remove(&name) {
				Some(tx) => {
					// Existing listing
					active.insert(name, tx);
				}
				None => {
					// New listing
					let (tx, rx) = oneshot::channel();
					active.insert(name.clone(), tx);
					self.pending.push_back(Listing { name, closed: rx });
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

		while let Some(payload) = group.read_frame().await? {
			if payload.is_empty() {
				anyhow::bail!("empty payload");
			}

			let (delta, name) = payload.split_at(1);
			let name = String::from_utf8_lossy(name).to_string();

			match delta[0] {
				b'+' => {
					// New listing
					let (tx, rx) = oneshot::channel();
					if self.active.insert(name.clone(), tx).is_some() {
						anyhow::bail!("duplicate listing");
					}
					return Ok(Some(Listing { name, closed: rx }));
				}
				b'-' => {
					// Removed listing
					self.active.remove(&name).context("non-existent listing")?;
				}
				_ => anyhow::bail!("invalid delta"),
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
	pub name: String,
	closed: oneshot::Receiver<()>,
}

impl Listing {
	pub async fn closed(self) -> anyhow::Result<()> {
		self.closed.await.ok();
		Ok(())
	}
}
