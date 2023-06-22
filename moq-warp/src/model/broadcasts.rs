use super::{broadcast, watch};

use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub enum Delta {
	// Pair was added
	Insert(String, broadcast::Subscriber),

	// Pair was removed
	Remove(String),
}

// TODO why does this need to be a tokio Mutex?
pub type Shared = Arc<Mutex<Publisher>>;

// Wrapper around a HashMap that publishes updates to a watch::Publisher.
#[derive(Default)]
pub struct Publisher {
	broadcasts: HashMap<String, broadcast::Subscriber>,
	updates: watch::Publisher<Delta>,
}

impl Publisher {
	pub fn updates(&self) -> watch::Subscriber<Delta> {
		self.updates.subscribe()
	}

	pub fn insert(&mut self, k: String, v: broadcast::Subscriber) {
		let existing = self.broadcasts.insert(k.clone(), v.clone());
		if existing.is_some() {
			self.updates.push(Delta::Remove(k.clone()));
		}
		self.updates.push(Delta::Insert(k, v));
	}

	pub fn remove(&mut self, k: &String) {
		let existing = self.broadcasts.remove(k);
		if existing.is_some() {
			self.updates.push(Delta::Remove(k.clone()));
		}
	}
}

impl Deref for Publisher {
	type Target = HashMap<String, broadcast::Subscriber>;

	fn deref(&self) -> &Self::Target {
		&self.broadcasts
	}
}
