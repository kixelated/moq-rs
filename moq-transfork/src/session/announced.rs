use std::collections::{HashMap, VecDeque};
use tokio::sync::watch;

use crate::BroadcastConsumer;

#[derive(Default)]
struct AnnouncedState {
	items: VecDeque<Option<BroadcastConsumer>>,
	index: HashMap<String, usize>,

	pruned: usize,
}

#[derive(Default, Clone)]
pub(super) struct AnnouncedProducer {
	state: watch::Sender<AnnouncedState>,
}

impl AnnouncedProducer {
	pub fn subscribe(&self) -> Announced {
		Announced {
			state: self.state.subscribe(),
			index: 0,
		}
	}

	pub fn insert(&self, broadcast: BroadcastConsumer) {
		self.state.send_modify(|state| {
			if let Some(old) = state.index.insert(broadcast.name.clone(), state.items.len()) {
				state.items[old - state.pruned] = None;
			}

			state.items.push_back(Some(broadcast.clone()));
		});
	}

	pub fn remove(&self, name: &str) {
		self.state.send_if_modified(|state| {
			if let Some(index) = state.index.remove(name) {
				state.items[index - state.pruned] = None;
				if index == state.pruned {
					self.prune();
				}
			}

			false
		});
	}

	// Called when we remove the first broadcast
	fn prune(&self) {
		self.state.send_if_modified(|state| {
			while let Some(None) = state.items.front() {
				state.items.pop_front();
			}

			false
		});
	}
}

pub struct Announced {
	state: watch::Receiver<AnnouncedState>,
	index: usize,
}

impl Announced {
	pub async fn next(&mut self) -> Option<BroadcastConsumer> {
		loop {
			{
				let state = self.state.borrow_and_update();

				if self.index < state.pruned {
					self.index = state.pruned;
				}

				while self.index < state.items.len() + state.pruned {
					let index = self.index - state.pruned;
					self.index += 1;

					if let Some(reader) = &state.items[index] {
						return Some(reader.clone());
					}
				}
			}

			if self.state.changed().await.is_err() {
				return None;
			}
		}
	}
}
