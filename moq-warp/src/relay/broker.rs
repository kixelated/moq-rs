use crate::model::{track, watch};
use crate::source::Source;

use std::collections::hash_map::{HashMap};
use std::sync::{Arc, Mutex};

use anyhow::Context;

#[derive(Clone, Default)]
pub struct Broadcasts {
	// Operate on the inner struct so we can share/clone the outer struct.
	inner: Arc<Mutex<BroadcastsInner>>,
}

#[derive(Default)]
struct BroadcastsInner {
	// TODO Automatically reclaim dropped sources.
	lookup: HashMap<String, Arc<dyn Source + Send + Sync>>,
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

	pub fn announce(&self, namespace: &str, source: Arc<dyn Source + Send + Sync>) -> anyhow::Result<()> {
		let mut this = self.inner.lock().unwrap();

		if let Some(_existing) = this.lookup.get(namespace) {
			anyhow::bail!("namespace already registered");
		}

		this.lookup.insert(namespace.to_string(), source);

		Ok(())
	}

	pub fn unannounce(&self, namespace: &str) -> anyhow::Result<()> {
		let mut this = self.inner.lock().unwrap();
		this.lookup.remove(namespace).context("namespace was not published")?;
		Ok(())
	}

	pub fn subscribe(&self, namespace: &str, name: &str) -> Option<track::Subscriber> {
		let this = self.inner.lock().unwrap();

		this.lookup.get(namespace).and_then(|v| v.subscribe(name))
	}
}
