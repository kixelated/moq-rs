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
	routes: Arc<Mutex<HashMap<Path, Vec<Option<Session>>>>>,
}

impl Default for Origins {
	fn default() -> Self {
		Self::new()
	}
}

impl Origins {
	pub fn new() -> Self {
		Self {
			unique: AnnouncedProducer::new(),
			routes: Default::default(),
		}
	}

	// Route any announcements from the cluster.
	pub async fn announce(&mut self, mut announced: AnnouncedConsumer, origin: Option<Session>) {
		while let Some(announced) = announced.next().await {
			match announced {
				Announced::Active(path) => self.announce_track(path, origin.clone()),
				Announced::Ended(path) => self.unannounce_track(path, &origin),
				Announced::Live => {
					// Ignore.
				}
			}
		}
	}

	fn announce_track(&mut self, path: Path, origin: Option<Session>) {
		tracing::info!(?path, "announced origin");

		let mut routes = self.routes.lock().unwrap();
		match routes.entry(path.clone()) {
			hash_map::Entry::Occupied(mut entry) => entry.get_mut().push(origin),
			hash_map::Entry::Vacant(entry) => {
				entry.insert(vec![origin]);
				self.unique.announce(path);
			}
		}
	}

	fn unannounce_track(&mut self, path: Path, origin: &Option<Session>) {
		tracing::info!(?path, "unannounced origin");

		let mut routes = self.routes.lock().unwrap();
		let entry = match routes.entry(path.clone()) {
			hash_map::Entry::Occupied(entry) => entry.into_mut(),
			hash_map::Entry::Vacant(_) => return,
		};

		// Technically this is wrong, as it will remove more than one None value.
		// But currently there can only be one None that will never be removed, so it's fine.
		entry.retain(|s| s != origin);

		if entry.is_empty() {
			routes.remove(&path);
			self.unique.unannounce(&path);
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
		let routes = self.routes.lock().unwrap();

		let available = routes.get(path)?;
		available.iter().find(|route| route.is_some()).cloned().unwrap()
	}
}
