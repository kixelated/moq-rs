use anyhow::Context;
use bytes::BytesMut;
use std::collections::{HashSet, VecDeque};

use moq_transfork::prelude::*;

pub struct ListingProducer {
	track: Option<TrackProducer>,
	groups: Option<TrackProducer>,
	group: Option<GroupProducer>,

	current: HashSet<String>,
}

impl ListingProducer {
	pub fn new(track: TrackProducer) -> Self {
		Self {
			track: Some(track),
			groups: None,
			group: None,
			current: HashSet::new(),
		}
	}

	pub fn insert(&mut self, name: String) -> Result<(), moq_transfork::Error> {
		if !self.current.insert(name.clone()) {
			return Err(moq_transfork::Error::Duplicate);
		}

		match self.group {
			// Create a delta if the current group is small enough.
			Some(ref mut group) if self.current.len() < 2 * group.frame_count() => {
				let msg = format!("+{}", name);
				group.write_frame(msg.into());
			}
			// Otherwise create a snapshot with every element.
			_ => self.group = Some(self.snapshot()?),
		}

		Ok(())
	}

	pub fn remove(&mut self, name: &str) -> moq_transfork::Result<()> {
		if !self.current.remove(name) {
			// TODO this is a wrong error message.
			return Err(moq_transfork::Error::NotFound);
		}

		match self.group {
			// Create a delta if the current group is small enough.
			Some(ref mut group) if self.current.len() < 2 * group.frame_count() => {
				let msg = format!("-{}", name);
				group.write_frame(msg.into());
			}
			// Otherwise create a snapshot with every element.
			_ => self.group = Some(self.snapshot()?),
		}

		Ok(())
	}

	fn snapshot(&mut self) -> Result<GroupProducer, moq_transfork::Error> {
		let mut groups = match self.groups.take() {
			Some(groups) => groups,
			None => self.track.take().unwrap(),
		};

		let mut group = groups.append_group();

		let mut msg = BytesMut::new();
		for name in &self.current {
			msg.extend_from_slice(name.as_bytes());
			msg.extend_from_slice(b"\n");
		}

		group.write_frame(msg.freeze());
		self.groups = Some(groups);

		Ok(group)
	}

	pub fn len(&self) -> usize {
		self.current.len()
	}

	pub fn is_empty(&self) -> bool {
		self.current.is_empty()
	}
}

#[derive(Clone)]
pub enum ListingDelta {
	Add(String),
	Rem(String),
}

#[derive(Clone)]
pub struct ListingConsumer {
	track: TrackConsumer,

	// Keep track of the current group.
	group: Option<GroupConsumer>,

	// The current state of the listing.
	current: HashSet<String>,

	// A list of deltas we need to return
	deltas: VecDeque<ListingDelta>,
}

impl ListingConsumer {
	pub fn new(track: TrackConsumer) -> Self {
		Self {
			track,
			group: None,

			current: HashSet::new(),
			deltas: VecDeque::new(),
		}
	}

	pub async fn next(&mut self) -> anyhow::Result<Option<ListingDelta>> {
		if let Some(delta) = self.deltas.pop_front() {
			return Ok(Some(delta));
		}

		if self.group.is_none() {
			self.group = Some(self.track.next_group().await?.context("empty track")?);
		}

		let mut group_done = false;
		let mut groups_done = false;

		loop {
			tokio::select! {
				next = self.track.next_group(), if !groups_done => {
					if let Some(next) = next? {
						self.group = Some(next);
						group_done = false;
					} else {
						groups_done = true;
					}
				},
				object = self.group.as_mut().unwrap().read_frame(), if !group_done => {
					let payload = match object? {
						Some(object) => object,
						None => {
							group_done = true;
							continue;
						}
					};

					if payload.is_empty() {
						anyhow::bail!("empty payload");
					} else if self.group.as_mut().unwrap().frame_index() == 1 {
						// This is a full snapshot, not a delta
						let set = HashSet::from_iter(payload.split(|&b| b == b'\n').map(|s| String::from_utf8_lossy(s).to_string()));

						for name in set.difference(&self.current) {
							self.deltas.push_back(ListingDelta::Add(name.clone()));
						}

						for name in self.current.difference(&set) {
							self.deltas.push_back(ListingDelta::Rem(name.clone()));
						}

						self.current = set;

						if let Some(delta) = self.deltas.pop_front() {
							return Ok(Some(delta));
						}
					} else if payload[0] == b'+' {
						return Ok(Some(ListingDelta::Add(String::from_utf8_lossy(&payload[1..]).to_string())));
					} else if payload[0] == b'-' {
						return Ok(Some(ListingDelta::Rem(String::from_utf8_lossy(&payload[1..]).to_string())));
					} else {
						anyhow::bail!("invalid delta: {:?}", payload);
					}
				}
				else => return Ok(None),
			}
		}
	}

	// If you just want to proxy the track
	pub fn into_inner(self) -> TrackConsumer {
		self.track
	}
}
