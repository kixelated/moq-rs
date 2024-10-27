use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use moq_transfork::{Announced, AnnouncedConsumer, AnnouncedProducer, Path, Session};

#[derive(Clone)]
pub struct Origins {
	// Tracks announced by clients.
	unique: AnnouncedProducer,

	// Active routes based on path.
	routes: Arc<Mutex<HashMap<Path, Vec<Session>>>>,
}

impl Default for Origins {
	fn default() -> Self {
		Self::new()
	}
}

impl Origins {
	pub fn new() -> Self {
		Self {
			unique: AnnouncedProducer::new(1024),
			routes: Default::default(),
		}
	}

	pub async fn publish(&mut self, origin: Session) {
		let mut announced = origin.announced();

		while let Some(announced) = announced.next().await {
			match announced {
				Announced::Active(path) => {
					let mut routes = self.routes.lock().unwrap();
					match routes.entry(path.clone()) {
						hash_map::Entry::Occupied(mut entry) => entry.get_mut().push(origin.clone()),
						hash_map::Entry::Vacant(entry) => {
							entry.insert(vec![origin.clone()]);
							self.unique.announce(path.clone());
						}
					}
				}
				Announced::Ended(path) => {
					let mut routes = self.routes.lock().unwrap();
					let entry = match routes.entry(path.clone()) {
						hash_map::Entry::Occupied(entry) => entry.into_mut(),
						hash_map::Entry::Vacant(_) => continue,
					};

					entry.retain(|s| s != &origin);

					if entry.is_empty() {
						routes.remove(&path);
						self.unique.unannounce(&path);
					}
				}
			}
		}
	}

	pub fn announced(&self) -> AnnouncedConsumer {
		self.unique.subscribe()
	}

	pub fn announced_prefix(&self, prefix: Path) -> AnnouncedConsumer {
		self.unique.subscribe_prefix(prefix)
	}

	pub fn route(&self, path: &Path) -> Option<Session> {
		// Return the session that most recently announced the path.
		self.routes
			.lock()
			.unwrap()
			.get(path)
			.map(|list| list.last().cloned().unwrap())
	}
}
