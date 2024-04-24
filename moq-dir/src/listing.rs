use anyhow::Context;
use bytes::BytesMut;
use std::{
	collections::{HashMap, HashSet, VecDeque},
	sync::{Arc, Mutex},
};

use moq_transport::serve::{
	GroupReader, GroupWriter, GroupsReader, GroupsWriter, ServeError, Track, TrackReader, TrackReaderMode, TrackWriter,
};

#[derive(Clone)]
pub struct Listings {
	// Our namespace
	namespace: String,

	// A map of tracks currently being produced.
	lookup: Arc<Mutex<HashMap<String, (ListingWriter, ListingReader)>>>,
}

impl Listings {
	pub fn new(namespace: String) -> Self {
		Self {
			namespace,
			lookup: Default::default(),
		}
	}

	// Returns a Registration that removes on drop.
	pub fn register(&mut self, path: &str) -> Option<Registration> {
		let (prefix, base) = Self::bucket(path);

		if !prefix.starts_with(&self.namespace) {
			// Ignore anything that isn't in our namespace.
			return None;
		}

		// Remove the namespace prefix from the path.
		let prefix = &prefix[self.namespace.len()..];

		let mut lookup = self.lookup.lock().unwrap();

		let listing = lookup.entry(prefix.to_string()).or_insert_with(|| {
			let (writer, reader) = Track::new(self.namespace.clone(), prefix.to_string()).produce();
			(ListingWriter::new(writer), ListingReader::new(reader))
		});

		listing.0.insert(base.to_string()).unwrap();

		Some(Registration {
			listing: self.clone(),
			prefix: prefix.to_string(),
			base: base.to_string(),
		})
	}

	pub fn subscribe(&self, namespace: &str, name: &str) -> Result<ListingReader, ServeError> {
		if namespace == self.namespace {
			if let Some(listing) = self.lookup.lock().unwrap().get(name) {
				return Ok(listing.1.clone());
			}
		}

		Err(ServeError::NotFound)
	}

	fn remove(&mut self, prefix: &str, base: &str) -> Result<(), ServeError> {
		let mut lookup = self.lookup.lock().unwrap();

		let listing = lookup.get_mut(prefix).ok_or(ServeError::NotFound)?;
		listing.0.remove(base)?;

		if listing.0.is_empty() {
			lookup.remove(prefix);
		}

		Ok(())
	}

	// Returns the prefix for the string.
	// This is just the content before the last '/', like a directory name.
	// ex. "/foo/bar/baz" -> ("/foo/bar", "baz")
	pub fn bucket(path: &str) -> (&str, &str) {
		// Find the last '/' and return the parts.
		match path.rfind('/') {
			Some(index) => (&path[..index + 1], &path[index + 1..]),
			None => (path, ""),
		}
	}
}

pub struct ListingWriter {
	track: Option<TrackWriter>,
	groups: Option<GroupsWriter>,
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

	pub fn insert(&mut self, name: String) -> Result<(), ServeError> {
		if !self.current.insert(name.clone()) {
			return Err(ServeError::Duplicate);
		}

		match self.group {
			// Create a delta if the current group is small enough.
			Some(ref mut group) if self.current.len() < 2 * group.len() => {
				let msg = format!("+{}", name);
				group.write(msg.into())?;
			}
			// Otherwise create a snapshot with every element.
			_ => self.group = Some(self.snapshot()?),
		}

		Ok(())
	}

	pub fn remove(&mut self, name: &str) -> Result<(), ServeError> {
		if !self.current.remove(name) {
			return Err(ServeError::NotFound);
		}

		match self.group {
			// Create a delta if the current group is small enough.
			Some(ref mut group) if self.current.len() < 2 * group.len() => {
				let msg = format!("-{}", name);
				group.write(msg.into())?;
			}
			// Otherwise create a snapshot with every element.
			_ => self.group = Some(self.snapshot()?),
		}

		Ok(())
	}

	fn snapshot(&mut self) -> Result<GroupWriter, ServeError> {
		let mut groups = match self.groups.take() {
			Some(groups) => groups,
			None => self.track.take().unwrap().groups()?,
		};

		let priority = self.group.as_ref().map(|g| g.group_id + 1).unwrap_or(0);
		let mut group = groups.append(priority)?;

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
	Done,
}

#[derive(Clone)]
pub struct ListingReader {
	track: TrackReader,

	// Keep track of the current group.
	groups: Option<GroupsReader>,
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
			groups: None,
			group: None,

			current: HashSet::new(),
			deltas: VecDeque::new(),
		}
	}

	pub async fn next(&mut self) -> anyhow::Result<ListingDelta> {
		if let Some(delta) = self.deltas.pop_front() {
			return Ok(delta);
		}

		if self.groups.is_none() {
			self.groups = match self.track.mode().await? {
				TrackReaderMode::Groups(groups) => Some(groups),
				_ => anyhow::bail!("expected groups mode"),
			};
		};

		if self.group.is_none() {
			self.group = Some(self.groups.as_mut().unwrap().next().await?.context("empty track")?);
		}

		let mut group_done = false;
		let mut groups_done = false;

		loop {
			tokio::select! {
				next = self.groups.as_mut().unwrap().next(), if !groups_done => {
					if let Some(next) = next? {
						self.group = Some(next);
						group_done = false;
					} else {
						groups_done = true;
					}
				},
				object = self.group.as_mut().unwrap().read_next(), if !group_done => {
					let payload = match object? {
						Some(object) => object,
						None => {
							group_done = true;
							continue;
						}
					};

					if payload.is_empty() {
						anyhow::bail!("empty payload");
					} else if self.group.as_mut().unwrap().pos() == 1 {
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
							return Ok(delta);
						}
					} else if payload[0] == b'+' {
						return Ok(ListingDelta::Add(String::from_utf8_lossy(&payload[1..]).to_string()));
					} else if payload[0] == b'-' {
						return Ok(ListingDelta::Rem(String::from_utf8_lossy(&payload[1..]).to_string()));
					} else {
						anyhow::bail!("invalid delta: {:?}", payload);
					}
				}
			}
		}
	}

	// If you just want to proxy the track
	pub fn into_inner(self) -> TrackReader {
		self.track
	}
}

// Used to remove the registration on drop.
pub struct Registration {
	listing: Listings,
	prefix: String,
	base: String,
}

impl Drop for Registration {
	fn drop(&mut self) {
		self.listing.remove(&self.prefix, &self.base).ok();
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_bucket() {
		assert!(Listings::bucket("/") == ("/", ""));
		assert!(Listings::bucket("/foo") == ("/", "foo"));
		assert!(Listings::bucket("/foo/") == ("/foo/", ""));
		assert!(Listings::bucket("/foo/bar") == ("/foo/", "bar"));
		assert!(Listings::bucket("/foo/bar/") == ("/foo/bar/", ""));
		assert!(Listings::bucket("/foo/bar/baz") == ("/foo/bar/", "baz"));
		assert!(Listings::bucket("/foo/bar/baz/") == ("/foo/bar/baz/", ""));

		assert!(Listings::bucket("") == ("", ""));
		assert!(Listings::bucket("foo") == ("", "foo"));
		assert!(Listings::bucket("foo/") == ("foo/", ""));
		assert!(Listings::bucket("foo/bar") == ("foo/", "bar"));
		assert!(Listings::bucket("foo/bar/") == ("foo/bar/", ""));
		assert!(Listings::bucket("foo/bar/baz") == ("foo/bar/", "baz"));
		assert!(Listings::bucket("foo/bar/baz/") == ("foo/bar/baz/", ""));
	}
}
