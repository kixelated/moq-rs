use crate::model::{broadcast, track, watch};
use crate::relay::contribute;

use std::collections::hash_map::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Context;

#[derive(Clone, Default)]
pub struct Broker {
	// Operate on the inner struct so we can share/clone the outer struct.
	inner: Arc<Mutex<BrokerInner>>,
}

#[derive(Default)]
struct BrokerInner {
	// TODO Automatically reclaim dropped sources.
	lookup: HashMap<String, Arc<contribute::Broadcast>>,
	updates: watch::Publisher<BrokerUpdate>,
}

#[derive(Clone)]
pub enum BrokerUpdate {
	// Broadcast was announced
	Insert(String), // TODO include source?

	// Broadcast was unannounced
	Remove(String, broadcast::Error),
}

impl Broker {
	pub fn new() -> Self {
		Default::default()
	}

	// Return the list of available broadcasts, and a subscriber that will return updates (add/remove).
	pub fn available(&self) -> (Vec<String>, watch::Subscriber<BrokerUpdate>) {
		// Grab the lock.
		let this = self.inner.lock().unwrap();

		// Get the list of all available tracks.
		let keys = this.lookup.keys().cloned().collect();

		// Get a subscriber that will return future updates.
		let updates = this.updates.subscribe();

		(keys, updates)
	}

	pub fn announce(&self, namespace: &str, source: Arc<contribute::Broadcast>) -> anyhow::Result<()> {
		let mut this = self.inner.lock().unwrap();

		if let Some(_existing) = this.lookup.get(namespace) {
			anyhow::bail!("namespace already registered");
		}

		this.lookup.insert(namespace.to_string(), source);
		this.updates.push(BrokerUpdate::Insert(namespace.to_string()));

		Ok(())
	}

	pub fn unannounce(&self, namespace: &str, error: broadcast::Error) -> anyhow::Result<()> {
		let mut this = self.inner.lock().unwrap();

		this.lookup.remove(namespace).context("namespace was not published")?;
		this.updates.push(BrokerUpdate::Remove(namespace.to_string(), error));

		Ok(())
	}

	pub fn subscribe(&self, namespace: &str, name: &str) -> Option<track::Subscriber> {
		let this = self.inner.lock().unwrap();
		this.lookup.get(namespace).and_then(|v| v.subscribe(name))
	}
}
