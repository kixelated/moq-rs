use std::collections::{HashMap, VecDeque};
use tokio::sync::watch;

use crate::{Broadcast, BroadcastConsumer, Error};

use super::Publisher;

#[derive(Default)]
struct AnnouncedState {
	order: VecDeque<Option<BroadcastConsumer>>,
	lookup: HashMap<Broadcast, BroadcastConsumer>,
	pruned: usize,
}

#[derive(Default, Clone)]
pub struct AnnouncedProducer {
	state: watch::Sender<AnnouncedState>,
}

impl AnnouncedProducer {
	#[must_use = "removed on drop"]
	pub fn insert(&mut self, broadcast: BroadcastConsumer) -> Result<AnnouncedActive, Error> {
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

	pub fn get(&self, broadcast: &Broadcast) -> Option<BroadcastConsumer> {
		self.state.borrow().lookup.get(broadcast).cloned()
	}

	fn remove(&mut self, broadcast: &Broadcast, index: usize) {
		self.state.send_if_modified(|state| {
			state.lookup.remove(&broadcast).expect("broadcast not found");
			state.order[index - state.pruned] = None;

			while let Some(None) = state.order.front() {
				state.order.pop_front();
				state.pruned += 1;
			}

			false
		});
	}

	// Resolves when there are no subscribers
	pub async fn closed(&self) {
		self.state.closed().await
	}

	pub fn subscribe(&self) -> AnnouncedConsumer {
		AnnouncedConsumer {
			state: self.state.subscribe(),
			prefix: "".to_string(),
			index: 0,
		}
	}

	pub fn subscribe_prefix<P: ToString>(&self, prefix: P) -> AnnouncedConsumer {
		AnnouncedConsumer {
			state: self.state.subscribe(),
			prefix: prefix.to_string(),
			index: 0,
		}
	}
}

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

#[derive(Clone)]
pub struct AnnouncedConsumer {
	state: watch::Receiver<AnnouncedState>,
	prefix: String,
	index: usize,
}

impl AnnouncedConsumer {
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
						if announced.info.name.starts_with(&self.prefix) {
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

	pub async fn forward(mut self, mut publisher: Publisher) -> Result<(), Error> {
		while let Some(broadcast) = self.next().await {
			publisher.publish(broadcast)?;
		}

		Ok(())
	}

	pub fn prefix(&self) -> &str {
		&self.prefix
	}
}
