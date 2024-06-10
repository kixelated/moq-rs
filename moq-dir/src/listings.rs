use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};

use moq_transfork::{Broadcast, BroadcastReader, BroadcastWriter, ServeError};

use crate::ListingWriter;

struct State {
	active: HashMap<String, ListingWriter>,
	writer: BroadcastWriter,
}

#[derive(Clone)]
pub struct Listings {
	state: Arc<Mutex<State>>,
	reader: BroadcastReader,
}

impl Listings {
	pub fn new(broadcast: Broadcast) -> Self {
		let (writer, reader) = broadcast.produce();

		let state = State {
			active: Default::default(),
			writer,
		};

		Self {
			state: Arc::new(Mutex::new(state)),
			reader,
		}
	}

	// Returns a Registration that removes on drop.
	pub fn register(&mut self, path: &str) -> Result<Option<Registration>, ServeError> {
		let (prefix, base) = Self::prefix(path);

		if !prefix.starts_with(&self.reader.name) {
			// Ignore anything that isn't in our broadcast.
			return Ok(None);
		}

		// Remove the broadcast prefix from the path.
		let prefix = &prefix[self.reader.name.len()..];

		let mut state = self.state.lock().unwrap();
		if let Some(listing) = state.active.get_mut(prefix) {
			listing.insert(base.to_string())?;
		} else {
			log::info!("creating prefix: {}", prefix);
			let track = state.writer.create(prefix).build().unwrap();

			let mut listing = ListingWriter::new(track);
			listing.insert(base.to_string())?;
			state.active.insert(prefix.to_string(), listing);
		}

		log::info!("added listing: {} {}", prefix, base);

		Ok(Some(Registration {
			listing: self.clone(),
			prefix: prefix.to_string(),
			base: base.to_string(),
		}))
	}

	fn remove(&mut self, prefix: &str, base: &str) -> Result<(), ServeError> {
		let mut state = self.state.lock().unwrap();

		let listing = state.active.get_mut(prefix).ok_or(ServeError::NotFound)?;
		listing.remove(base)?;

		log::info!("removed listing: {} {}", prefix, base);

		if listing.is_empty() {
			log::info!("removed prefix: {}", prefix);
			state.active.remove(prefix);
			state.writer.remove(prefix);
		}

		Ok(())
	}

	// Returns the prefix for the string.
	// This is just the content before the last '/', like a directory name.
	// ex. "/foo/bar/baz" -> ("/foo/bar", "baz")
	pub fn prefix(path: &str) -> (&str, &str) {
		// Find the last '/' and return the parts.
		match path.rfind('.') {
			Some(index) => (&path[..index + 1], &path[index + 1..]),
			None => ("", path),
		}
	}

	pub fn broadcast(&self) -> BroadcastReader {
		self.reader.clone()
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
		assert!(Listings::prefix(".") == (".", ""));
		assert!(Listings::prefix(".foo") == (".", "foo"));
		assert!(Listings::prefix(".foo.") == (".foo.", ""));
		assert!(Listings::prefix(".foo.bar") == (".foo.", "bar"));
		assert!(Listings::prefix(".foo.bar.") == (".foo.bar.", ""));
		assert!(Listings::prefix(".foo.bar.baz") == (".foo.bar.", "baz"));
		assert!(Listings::prefix(".foo.bar.baz.") == (".foo.bar.baz.", ""));

		assert!(Listings::prefix("") == ("", ""));
		assert!(Listings::prefix("foo") == ("", "foo"));
		assert!(Listings::prefix("foo.") == ("foo.", ""));
		assert!(Listings::prefix("foo.bar") == ("foo.", "bar"));
		assert!(Listings::prefix("foo.bar.") == ("foo.bar.", ""));
		assert!(Listings::prefix("foo.bar.baz") == ("foo.bar.", "baz"));
		assert!(Listings::prefix("foo.bar.baz.") == ("foo.bar.baz.", ""));
	}
}
