use indexmap::IndexMap;
use tokio::sync::watch;

use crate::BroadcastConsumer;

#[derive(Default)]
struct AnnouncedState {
	broadcasts: IndexMap<String, Option<BroadcastConsumer>>,
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
			state.broadcasts.insert(broadcast.name.clone(), Some(broadcast));
		});
	}

	pub fn remove(&self, name: &str) {
		self.state.send_if_modified(|state| {
			if let Some(index) = state.broadcasts.get_index_of(name) {
				*state.broadcasts.get_index_mut(index).unwrap().1 = None;
				if index == 0 {
					self.prune();
				}
			}

			false
		});
	}

	// Called when we remove the first broadcast
	fn prune(&self) {
		let mut index = 0;

		let state = self.state.borrow();
		for i in 1..state.broadcasts.len() {
			if state.broadcasts.get_index(i).unwrap().1.is_some() {
				index = i;
				break;
			}
		}

		self.state.send_if_modified(|state| {
			state.broadcasts.drain(..index);
			state.pruned += index - 1;
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

				while self.index < state.broadcasts.len() + state.pruned {
					let index = self.index - state.pruned;
					self.index += 1;

					let (_, reader) = state.broadcasts.get_index(index).unwrap();

					if let Some(reader) = reader {
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
