use std::collections::{HashMap, VecDeque};
use tokio::sync::watch;

use crate::{Broadcast, BroadcastConsumer, Error};

use super::Path;

#[derive(Default)]
struct AnnouncedState {
	order: VecDeque<Option<BroadcastConsumer>>,
	lookup: HashMap<Broadcast, BroadcastConsumer>,
	pruned: usize,
}

/// Announces broadcasts to consumers over the network.
#[derive(Default, Clone)]
pub struct AnnouncedProducer {
	state: watch::Sender<AnnouncedState>,
}

impl AnnouncedProducer {
	/// Announce a broadcast, returning a guard that will remove it when dropped.
	#[must_use = "removed on drop"]
	pub fn insert(&self, broadcast: BroadcastConsumer) -> Result<AnnouncedActive, Error> {
		let mut index = 0;

		let ok = self.state.send_if_modified(|state| {
			if state.lookup.insert(broadcast.info.clone(), broadcast.clone()).is_none() {
				index = state.order.len() + state.pruned;
				state.order.push_back(Some(broadcast.clone()));

				true
			} else {
				false
			}
		});

		if !ok {
			return Err(Error::Duplicate);
		}

		Ok(AnnouncedActive {
			producer: self.clone(),
			broadcast,
			index,
		})
	}

	/// Returns a broadcast by name.
	pub fn get(&self, broadcast: &Broadcast) -> Option<BroadcastConsumer> {
		self.state.borrow().lookup.get(broadcast).cloned()
	}

	fn remove(&self, broadcast: &Broadcast, index: usize) {
		self.state.send_if_modified(|state| {
			state.lookup.remove(broadcast).expect("broadcast not found");
			state.order[index - state.pruned] = None;

			while let Some(None) = state.order.front() {
				state.order.pop_front();
				state.pruned += 1;
			}

			false
		});
	}

	/// Resolves when there are no subscribers
	pub async fn closed(&self) {
		self.state.closed().await
	}

	/// Subscribe to all announced broadcasts, including those already active.
	pub fn subscribe(&self) -> AnnouncedConsumer {
		AnnouncedConsumer {
			state: self.state.subscribe(),
			prefix: Path::default(),
			index: 0,
		}
	}

	/// Subscribe to all announced broadcasts based on a prefix, including those already active.
	pub fn subscribe_prefix(&self, prefix: Path) -> AnnouncedConsumer {
		AnnouncedConsumer {
			state: self.state.subscribe(),
			prefix,
			index: 0,
		}
	}
}

/// An announced broadcast, active until dropped.
pub struct AnnouncedActive {
	producer: AnnouncedProducer,
	broadcast: BroadcastConsumer,
	index: usize,
}

impl Drop for AnnouncedActive {
	fn drop(&mut self) {
		self.producer.remove(&self.broadcast.info, self.index);
	}
}

/// Consumes announced broadcasts over the network matching an optional prefix.
#[derive(Clone)]
pub struct AnnouncedConsumer {
	state: watch::Receiver<AnnouncedState>,
	prefix: Path,
	index: usize,
}

impl AnnouncedConsumer {
	/// Returns the next announced broadcast.
	pub async fn next(&mut self) -> Option<BroadcastConsumer> {
		loop {
			{
				let state = self.state.borrow_and_update();

				if self.index < state.pruned {
					self.index = state.pruned;
				}

				while self.index < state.order.len() + state.pruned {
					let index = self.index - state.pruned;
					self.index += 1;

					if let Some(announced) = &state.order[index] {
						if announced.info.path.has_prefix(&self.prefix) {
							return Some(announced.clone());
						}
					}
				}
			}

			if self.state.changed().await.is_err() {
				return None;
			}
		}
	}

	/// Returns a broadcast by name.
	pub fn get(&self, broadcast: &Broadcast) -> Option<BroadcastConsumer> {
		self.state.borrow().lookup.get(broadcast).cloned()
	}

	/// Returns the prefix in use.
	pub fn prefix(&self) -> &Path {
		&self.prefix
	}

	/// Make a new consumer with a different prefix.
	pub fn with_prefix(&self, prefix: Path) -> Self {
		Self {
			state: self.state.clone(),
			prefix,
			index: 0,
		}
	}
}
