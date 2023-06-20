use super::{watch, Broadcast};

use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub enum Delta {
	// Pair was added
	Insert(String, Broadcast),

	// Pair was removed
	Remove(String),
}

pub type Shared = Arc<Mutex<Publisher>>;

// Wrapper around a HashMap that publishes updates to a watch::Publisher.
#[derive(Default)]
pub struct Publisher {
	map: HashMap<String, Broadcast>,
	updates: watch::Publisher<Delta>,
}

impl Publisher {
	pub fn updates(&self) -> watch::Subscriber<Delta> {
		self.updates.subscribe()
	}

	pub fn insert(&mut self, k: String, v: Broadcast) {
		let existing = self.map.insert(k.clone(), v.clone());
		if existing.is_some() {
			self.updates.push(Delta::Remove(k.clone()));
		}
		self.updates.push(Delta::Insert(k, v));
	}

	pub fn remove(&mut self, k: &String) {
		let existing = self.map.remove(k);
		if existing.is_some() {
			self.updates.push(Delta::Remove(k.clone()));
		}
	}
}

impl Deref for Publisher {
	type Target = HashMap<String, Broadcast>;

	fn deref(&self) -> &Self::Target {
		&self.map
	}
}
