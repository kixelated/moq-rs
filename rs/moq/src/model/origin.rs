use std::collections::HashMap;

use crate::{AnnounceConsumer, AnnounceProducer, BroadcastConsumer};
use web_async::Lock;

/// A collection of broadcasts, published by potentially multiple clients.
#[derive(Clone, Default)]
pub struct Origin {
	// Tracks announced by clients.
	unique: AnnounceProducer,

	// Active broadcasts.
	routes: Lock<HashMap<String, BroadcastConsumer>>,
}

impl Origin {
	pub fn new() -> Self {
		Self::default()
	}

	/// Announce a broadcast, replacing the previous announcement if it exists.
	pub fn publish<T: ToString>(&mut self, path: T, broadcast: BroadcastConsumer) {
		let path = path.to_string();
		self.routes.lock().insert(path.clone(), broadcast.clone());
		self.unique.insert(&path);

		let mut this = self.clone();
		web_async::spawn(async move {
			// Wait until the broadcast is closed, then remove it from the lookup.
			broadcast.closed().await;

			// Remove the broadcast from the lookup only if it's not a duplicate.
			let mut routes = this.routes.lock();

			if let Some(existing) = routes.remove(&path) {
				if !existing.ptr_eq(&broadcast) {
					// Oops we were the duplicate, re-insert the original.
					routes.insert(path.to_string(), broadcast.clone());
				} else {
					// We were the original, remove from the unique set.
					this.unique.remove(&path);
				}
			}
		});
	}

	/// Consume a broadcast by path.
	pub fn consume(&self, path: &str) -> Option<BroadcastConsumer> {
		// Return the most recently announced broadcast
		self.routes.lock().get(path).cloned()
	}

	/// Discover any broadcasts published by the remote matching a prefix.
	///
	/// NOTE: The results contain the suffix only.
	pub fn announced(&self, prefix: &str) -> AnnounceConsumer {
		self.unique.consume(prefix)
	}
}
