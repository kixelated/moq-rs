use std::{
	collections::{HashSet, VecDeque},
	sync::{Arc, Mutex},
};
use tokio::sync::broadcast;

use crate::util::closed;

use super::Path;

#[derive(Clone)]
pub enum Announced {
	Active(Path),
	Ended(Path),
}

impl Announced {
	pub fn path(&self) -> &Path {
		match self {
			Announced::Active(path) => path,
			Announced::Ended(path) => path,
		}
	}
}

/// Announces tracks to consumers over the network.
#[derive(Clone)]
pub struct AnnouncedProducer {
	updates: broadcast::Sender<Announced>,
	active: Arc<Mutex<HashSet<Path>>>,
	closed: closed::Producer,
}

impl AnnouncedProducer {
	pub fn new(capacity: usize) -> Self {
		let (tx, _) = broadcast::channel(capacity);
		Self {
			updates: tx,
			active: Default::default(),
			closed: Default::default(),
		}
	}

	/// Announce a track, returning true if it's new.
	pub fn announce(&mut self, path: Path) -> bool {
		if self.active.lock().unwrap().insert(path.clone()) {
			let announced = Announced::Active(path);
			self.updates.send(announced).ok();
			true
		} else {
			false
		}
	}

	/// Stop announcing a track, returning true if it was active.
	pub fn unannounce(&mut self, path: &Path) -> bool {
		if self.active.lock().unwrap().remove(path) {
			let announced = Announced::Ended(path.clone());
			self.updates.send(announced).ok();
			true
		} else {
			false
		}
	}

	pub fn is_active(&self, path: &Path) -> bool {
		self.active.lock().unwrap().contains(path)
	}

	/// Subscribe to all announced tracks, including those already active.
	pub fn subscribe(&self) -> AnnouncedConsumer {
		self.subscribe_prefix(Path::default())
	}

	/// Subscribe to all announced tracks based on a prefix, including those already active.
	pub fn subscribe_prefix(&self, prefix: Path) -> AnnouncedConsumer {
		AnnouncedConsumer::new(
			prefix,
			self.active.clone(),
			self.updates.subscribe(),
			self.closed.subscribe(),
		)
	}

	pub async fn closed(&self) {
		self.closed.unused().await
	}
}

impl Default for AnnouncedProducer {
	fn default() -> Self {
		Self::new(32)
	}
}

/// Consumes announced tracks over the network matching an optional prefix.
pub struct AnnouncedConsumer {
	// The official list of active paths.
	active: Arc<Mutex<HashSet<Path>>>,

	// A set of updates that we haven't consumed yet.
	pending: VecDeque<Announced>,

	// A set of paths that we have consumed and must keep track of.
	tracked: HashSet<Path>,

	// New updates.
	updates: broadcast::Receiver<Announced>,

	// Only consume paths with this prefix.
	prefix: Path,

	// Used to notify the producer that there are no more consumers.
	// This would be a good thing to add to `broadcast::Sender` itself.
	_closed: closed::Consumer,
}

impl AnnouncedConsumer {
	fn new(
		prefix: Path,
		active: Arc<Mutex<HashSet<Path>>>,
		updates: broadcast::Receiver<Announced>,
		closed: closed::Consumer,
	) -> Self {
		let pending = active
			.lock()
			.unwrap()
			.iter()
			.filter(|path| path.has_prefix(&prefix))
			.cloned()
			.map(Announced::Active)
			.collect();

		Self {
			active,
			pending,
			updates,
			prefix,
			tracked: HashSet::new(),
			_closed: closed,
		}
	}

	/// Returns the next update.
	pub async fn next(&mut self) -> Option<Announced> {
		loop {
			// Remove any pending updates first.
			if let Some(announced) = self.pending.pop_front() {
				// Keep track of the fact that we returned this path.
				match &announced {
					Announced::Active(path) => self.tracked.insert(path.clone()),
					Announced::Ended(path) => self.tracked.remove(path),
				};

				return Some(announced);
			}

			// Get any new updates.
			match self.updates.recv().await {
				// We got a new update, but they're not filtered based on prefix.
				Ok(announced) => {
					match &announced {
						Announced::Active(path) => {
							if !path.has_prefix(&self.prefix) {
								// Wrong prefix.
								continue;
							}

							// Keep track of the fact that we returned this path.
							self.tracked.insert(path.clone());
						}
						Announced::Ended(path) => {
							if !self.tracked.remove(path) {
								// We don't care about this path (ex. wrong prefix)
								continue;
							}
						}
					};

					return Some(announced);
				}
				Err(broadcast::error::RecvError::Closed) => {
					// The producer has been closed, so we need to return Ended for all active paths.
					return match self.tracked.iter().next().cloned() {
						Some(path) => {
							self.tracked.remove(&path);
							Some(Announced::Ended(path))
						}
						None => None,
					};
				}
				Err(broadcast::error::RecvError::Lagged(_)) => {
					// We skipped a bunch of updates, so we need to resync.
					// Resubscribe to get the latest updates.
					self.updates.resubscribe();

					// Get the current list of active paths.
					let active: HashSet<Path> = self
						.active
						.lock()
						.unwrap()
						.iter()
						.filter(|path| path.has_prefix(&self.prefix))
						.cloned()
						.collect();

					// Figure out the deltas we need to apply to reach it.
					self.pending.clear();

					// Queue up any paths that we need to remove.
					for removed in self.tracked.difference(&active) {
						self.pending.push_back(Announced::Ended(removed.clone()));
					}

					// Queue up any paths that we need to add.
					for added in active.difference(&self.tracked) {
						self.pending.push_back(Announced::Active(added.clone()));
					}
				}
			}
		}
	}

	/// Returns the prefix in use.
	pub fn prefix(&self) -> &Path {
		&self.prefix
	}
}
