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
	}

	pub fn unpublish(&mut self, broadcast: &Broadcast) {
		self.routes.lock().remove(&broadcast);
		self.unique.remove(&broadcast);
	}

	pub fn consume(&self, broadcast: &Broadcast) -> Option<BroadcastConsumer> {
		// Return the most recently announced broadcast
		self.routes.lock().get(broadcast).cloned()
	}

	pub fn announced(&self, prefix: &str) -> AnnouncedConsumer {
		self.unique.consume(prefix)
	}
}
