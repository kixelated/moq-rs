use std::collections::{HashMap, VecDeque};
use tokio::sync::{mpsc, watch};

use crate::Error;

use super::Path;

#[derive(Default)]
struct AnnouncedState {
	order: VecDeque<Option<AnnouncedActive>>,
	active: HashMap<Path, mpsc::Receiver<()>>,
	pruned: usize,
}

/// Announces tracks to consumers over the network.
#[derive(Default, Clone)]
pub struct AnnouncedProducer {
	state: watch::Sender<AnnouncedState>,
}

impl AnnouncedProducer {
	/// Announce a track, returning a guard that will remove it when dropped.
	#[must_use = "removed on drop"]
	pub fn insert(&self, path: Path) -> Result<AnnouncedGuard, Error> {
		let mut index = 0;

		let (tx, rx) = mpsc::channel(1);
		let active = AnnouncedActive {
			path: path.clone(),
			closed: tx,
		};

		let ok = self.state.send_if_modified(|state| {
			if state.active.insert(path.clone(), rx).is_none() {
				index = state.order.len() + state.pruned;
				state.order.push_back(Some(active));

				true
			} else {
				false
			}
		});

		if !ok {
			return Err(Error::Duplicate);
		}

		Ok(AnnouncedGuard {
			producer: self.clone(),
			path,
			index,
		})
	}

	fn remove(&self, path: &Path, index: usize) {
		self.state.send_if_modified(|state| {
			state.active.remove(path);
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

	/// Subscribe to all announced tracks, including those already active.
	pub fn subscribe(&self) -> AnnouncedConsumer {
		AnnouncedConsumer {
			state: self.state.subscribe(),
			prefix: Path::default(),
			index: 0,
		}
	}

	/// Subscribe to all announced tracks based on a prefix, including those already active.
	pub fn subscribe_prefix(&self, prefix: Path) -> AnnouncedConsumer {
		AnnouncedConsumer {
			state: self.state.subscribe(),
			prefix,
			index: 0,
		}
	}
}

/// An announced track, active until dropped.
pub struct AnnouncedGuard {
	producer: AnnouncedProducer,
	path: Path,
	index: usize,
}

impl Drop for AnnouncedGuard {
	fn drop(&mut self) {
		self.producer.remove(&self.path, self.index);
	}
}

/// Consumes announced tracks over the network matching an optional prefix.
#[derive(Clone)]
pub struct AnnouncedConsumer {
	state: watch::Receiver<AnnouncedState>,
	prefix: Path,
	index: usize,
}

impl AnnouncedConsumer {
	/// Returns the next announced track.
	pub async fn next(&mut self) -> Option<AnnouncedActive> {
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
						if announced.path.has_prefix(&self.prefix) {
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

#[derive(Clone)]
pub struct AnnouncedActive {
	pub path: Path,
	closed: mpsc::Sender<()>,
}

impl AnnouncedActive {
	pub async fn closed(&self) {
		self.closed.closed().await
	}
}
