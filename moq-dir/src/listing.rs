use anyhow::Context;
use bytes::BytesMut;
use std::collections::{HashSet, VecDeque};

use moq_transfork::prelude::*;

pub struct ListingWriter {
	track: Option<TrackWriter>,
	groups: Option<TrackWriter>,
	group: Option<GroupWriter>,

	current: HashSet<String>,
}

impl ListingWriter {
	pub fn new(track: TrackWriter) -> Self {
		Self {
			track: Some(track),
			groups: None,
			group: None,
			current: HashSet::new(),
		}
	}

	pub fn insert(&mut self, name: String) -> Result<(), Closed> {
		if !self.current.insert(name.clone()) {
			return Err(Closed::Duplicate);
		}

		match self.group {
			// Create a delta if the current group is small enough.
			Some(ref mut group) if self.current.len() < 2 * group.total() => {
				let msg = format!("+{}", name);
				group.write(msg.into())?;
			}
			// Otherwise create a snapshot with every element.
			_ => self.group = Some(self.snapshot()?),
		}

		Ok(())
	}

	pub fn remove(&mut self, name: &str) -> Result<(), Closed> {
		if !self.current.remove(name) {
			// TODO this is a wrong error message.
			return Err(Closed::Unknown);
		}

		match self.group {
			// Create a delta if the current group is small enough.
			Some(ref mut group) if self.current.len() < 2 * group.total() => {
				let msg = format!("-{}", name);
				group.write(msg.into())?;
			}
			// Otherwise create a snapshot with every element.
			_ => self.group = Some(self.snapshot()?),
		}

		Ok(())
	}

	fn snapshot(&mut self) -> Result<GroupWriter, Closed> {
		let mut groups = match self.groups.take() {
			Some(groups) => groups,
			None => self.track.take().unwrap(),
		};

		let mut group = groups.append()?;

		let mut msg = BytesMut::new();
		for name in &self.current {
			msg.extend_from_slice(name.as_bytes());
			msg.extend_from_slice(b"\n");
		}

		group.write(msg.freeze())?;
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
pub struct ListingReader {
	track: TrackReader,

	// Keep track of the current group.
	group: Option<GroupReader>,

	// The current state of the listing.
	current: HashSet<String>,

	// A list of deltas we need to return
	deltas: VecDeque<ListingDelta>,
}

impl ListingReader {
	pub fn new(track: TrackReader) -> Self {
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
			self.group = Some(self.track.next().await?.context("empty track")?);
		}

		let mut group_done = false;
		let mut groups_done = false;

		loop {
			tokio::select! {
				next = self.track.next(), if !groups_done => {
					if let Some(next) = next? {
						self.group = Some(next);
						group_done = false;
					} else {
						groups_done = true;
					}
				},
				object = self.group.as_mut().unwrap().read(), if !group_done => {
					let payload = match object? {
						Some(object) => object,
						None => {
							group_done = true;
							continue;
						}
					};

					if payload.is_empty() {
						anyhow::bail!("empty payload");
					} else if self.group.as_mut().unwrap().current() == 1 {
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
	pub fn into_inner(self) -> TrackReader {
		self.track
	}
}
