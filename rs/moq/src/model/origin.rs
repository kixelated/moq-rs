use std::collections::HashMap;

use crate::{AnnouncedConsumer, AnnouncedProducer, Broadcast, BroadcastConsumer};
use web_async::Lock;

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
	pub fn publish(&mut self, broadcast: BroadcastConsumer) {
		self.routes.lock().insert(broadcast.info.clone(), broadcast.clone());
		self.unique.insert(broadcast.info.clone());

		let mut this = self.clone();
		web_async::spawn(async move {
			// Wait until the broadcast is closed, then remove it from the lookup.
			broadcast.closed().await;

			// Remove the broadcast from the lookup only if it's not a duplicate.
			let mut routes = this.routes.lock();

			if let Some(existing) = routes.remove(&broadcast.info) {
				if !existing.ptr_eq(&broadcast) {
					// Oops we were the duplicate, re-insert the original.
					routes.insert(broadcast.info.clone(), broadcast.clone());
				} else {
					// We were the original, remove from the unique set.
					this.unique.remove(&broadcast.info);
				}
			}
		});
	}

	pub fn consume(&self, broadcast: &Broadcast) -> Option<BroadcastConsumer> {
		// Return the most recently announced broadcast
		self.routes.lock().get(broadcast).cloned()
	}

	pub fn announced(&self, prefix: &str) -> AnnouncedConsumer {
		self.unique.consume(prefix)
	}
}
