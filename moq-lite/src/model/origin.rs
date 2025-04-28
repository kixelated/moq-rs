use std::collections::HashMap;

use crate::{AnnouncedConsumer, AnnouncedProducer, Broadcast, BroadcastConsumer};
use web_async::{spawn, Lock};

#[derive(Clone, Default)]
pub struct Origin {
	// Tracks announced by clients.
	unique: AnnouncedProducer,

	// Active broadcasts.
	routes: Lock<HashMap<Broadcast, BroadcastConsumer>>,
}

impl Origin {
	pub fn new() -> Self {
		Self::default()
	}

	// Announce a broadcast, replacing the previous announcement if it exists.
	pub fn publish(&mut self, broadcast: BroadcastConsumer) -> Option<BroadcastConsumer> {
		let mut routes = self.routes.lock();

		let existing = routes.insert(broadcast.info.clone(), broadcast.clone());
		if existing.is_some() {
			tracing::info!(broadcast = ?broadcast.info, "re-announced origin");

			// Reannounce as a signal that the origin changed.
			self.unique.remove(&broadcast.info);
		} else {
			tracing::info!(broadcast = ?broadcast.info, "announced origin");
		}

		self.unique.insert(broadcast.info.clone());

		// Spawn a background task to clean up the broadcast when it closes.
		let mut this = self.clone();
		spawn(async move {
			broadcast.closed().await;
			let mut routes = this.routes.lock();

			let existing = routes.remove(&broadcast.info).unwrap();
			if existing == broadcast {
				tracing::info!(broadcast = ?broadcast.info, "unannounced origin");
				this.unique.remove(&broadcast.info);
			} else {
				// Oops, put it back (we were a duplicate).
				routes.insert(broadcast.info, existing);
			}
		});

		existing
	}

	pub fn consume(&self, broadcast: &Broadcast) -> Option<BroadcastConsumer> {
		// Return the most recently announced broadcast
		self.routes.lock().get(broadcast).cloned()
	}

	pub fn announced(&self, prefix: &str) -> AnnouncedConsumer {
		self.unique.consume(prefix)
	}
}
