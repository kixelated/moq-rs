use super::{Shared, Watch};

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

#[derive(Clone, Debug)]
pub enum Delta<K, V> {
	// Pair was added
	Insert(K, V),

	// Pair was removed
	Remove(K),
}

#[derive(Clone)]
pub struct Publisher<K, V> {
	keys: HashSet<K>, // used to check for duplicates
	updates: Watch<Delta<K, V>>,
}

impl<K, V> Publisher<K, V>
where
	K: Clone + Eq + Hash,
	V: Clone,
{
	pub fn new(state: Shared<Delta<K, V>>) -> Self {
		Self {
			keys: HashSet::new(),
			updates: Watch::new(state),
		}
	}

	pub fn insert(&mut self, k: K, v: V) -> anyhow::Result<()> {
		let mut state = self.updates.lock(|delta| match delta {
			Delta::Insert(k, _v) => {
				self.keys.insert(k);
			}
			Delta::Remove(k) => {
				self.keys.remove(&k);
			}
		});

		if self.keys.contains(&k) {
			anyhow::bail!("key already exists");
		}

		state.push(Delta::Insert(k, v));

		Ok(())
	}

	pub fn remove(&mut self, k: K) -> anyhow::Result<()> {
		let mut state = self.updates.lock(|delta| match delta {
			Delta::Insert(k, _v) => {
				self.keys.insert(k);
			}
			Delta::Remove(k) => {
				self.keys.remove(&k);
			}
		});

		if !self.keys.contains(&k) {
			anyhow::bail!("key doesn't exist");
		}

		state.push(Delta::Remove(k));

		Ok(())
	}
}

#[derive(Clone)]
pub struct Subscriber<K, V> {
	current: HashMap<K, V>,
	updates: Watch<Delta<K, V>>,
}

impl<K, V> Subscriber<K, V>
where
	K: Clone + Eq + Hash,
	V: Clone,
{
	pub fn new(state: Shared<Delta<K, V>>) -> Self {
		Self {
			current: HashMap::new(),
			updates: Watch::new(state),
		}
	}

	pub fn current(&self) -> &HashMap<K, V> {
		&self.current
	}

	pub async fn next(&mut self) -> Delta<K, V> {
		let delta = self.updates.next().await;

		match &delta {
			Delta::Insert(k, v) => {
				self.current.insert(k.clone(), v.clone());
			}
			Delta::Remove(k) => {
				self.current.remove(k);
			}
		}

		delta
	}
}
