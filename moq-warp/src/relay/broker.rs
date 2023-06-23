use crate::model::{track, watch};
use crate::source::Source;

use std::collections::hash_map::{Entry, HashMap};
use std::sync::{Arc, Mutex};

use anyhow::Context;

#[derive(Clone, Default)]
pub struct Broadcasts {
	// Operate on the inner struct so we can share/clone the outer struct.
	inner: Arc<Mutex<BroadcastsInner>>,
}

#[derive(Default)]
struct BroadcastsInner {
	lookup: HashMap<String, Arc<Mutex<dyn Source + Send + Sync>>>,
	updates: watch::Publisher<Update>,
}

#[derive(Clone)]
pub enum Update {
	// Broadcast was announced
	Insert(String),

	// Broadcast was unannounced
	Remove(String),
}

impl Broadcasts {
	pub fn new() -> Self {
		Default::default()
	}

	// Return the list of available broadcasts, and a subscriber that will return updates (add/remove).
	pub fn available(&self) -> (Vec<String>, watch::Subscriber<Update>) {
		// Grab the lock.
		let this = self.inner.lock().unwrap();

		// Get the list of all available tracks.
		let keys = this.lookup.keys().cloned().collect();

		// Get a subscriber that will return updates.
		let updates = this.updates.subscribe();

		(keys, updates)
	}

	pub fn publish<T: Source + Send + Sync + 'static>(&self, namespace: &str, source: T) -> anyhow::Result<()> {
		let mut this = self.inner.lock().unwrap();

		let entry = match this.lookup.entry(namespace.into()) {
			Entry::Occupied(_) => anyhow::bail!("namespace already registered"),
			Entry::Vacant(entry) => entry,
		};

		entry.insert(Arc::new(Mutex::new(source)));
		Ok(())
	}

	pub fn unpublish(&self, namespace: &str) -> anyhow::Result<()> {
		let mut this = self.inner.lock().unwrap();
		this.lookup.remove(namespace).context("namespace was not published")?;
		Ok(())
	}

	pub fn subscribe(&self, namespace: &str, name: &str) -> Option<track::Subscriber> {
		let mut this = self.inner.lock().unwrap();
		match this.lookup.get_mut(namespace) {
			Some(source) => source.lock().unwrap().subscribe(name),
			None => None,
		}
	}
}
