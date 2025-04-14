use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};

use moq_lite::{AnnouncedConsumer, AnnouncedProducer, Broadcast, BroadcastConsumer, Session};
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct Origins {
	// Tracks announced by clients.
	unique: AnnouncedProducer,

	// Active routes based on path.
	routes: Arc<Mutex<HashMap<Broadcast, (BroadcastConsumer, JoinHandle<()>)>>>,
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

	// Announce a broadcast, replacing the previous announcement if it exists.
	pub fn announce(&mut self, broadcast: BroadcastConsumer) {
		let mut routes = self.routes.lock().unwrap();

		let broadcast2 = broadcast.clone();
		let routes2 = self.routes.clone();
		let mut unique2 = self.unique.clone();

		// TODO figure out a better way to do this.
		let cleanup = tokio::spawn(async move {
			broadcast2.closed().await;
			routes2.lock().unwrap().remove(&broadcast2.info);
			unique2.unannounce(&broadcast2.info);

			tracing::info!(broadcast = ?broadcast2.info.path, "unannounced origin");
		});

		if let Some(existing) = routes.insert(broadcast.info.clone(), (broadcast.clone(), cleanup)) {
			tracing::info!(broadcast = ?broadcast.info, "re-announced origin");
			existing.1.abort();
		} else {
			tracing::info!(broadcast = ?broadcast.info, "announced origin");
			self.unique.announce(broadcast.info);
		}
	}

	pub fn route(&self, broadcast: &Broadcast) -> Option<BroadcastConsumer> {
		// Return the session that most recently announced the path.
		let routes = self.routes.lock().unwrap();
		routes.get(broadcast).map(|(broadcast, _)| broadcast.clone())
	}

	// Subscribe to all broadcasts from the given session.
	pub async fn subscribe_from(&mut self, upstream: Session) {
		let mut announced = upstream.announced("");

		while let Some(broadcast) = announced.active().await {
			let broadcast = upstream.subscribe(broadcast);
			self.announce(broadcast);
		}
	}

	// Route all broadcasts to the given session
	pub async fn publish_to(&self, mut downstream: Session) -> anyhow::Result<()> {
		let mut remotes = self.unique.subscribe("");

		while let Some(broadcast) = remotes.active().await {
			if let Some(upstream) = self.route(&broadcast) {
				downstream.publish(upstream)?;
			}
		}

		Ok(())
	}

	pub fn announced(&self, prefix: &str) -> AnnouncedConsumer {
		self.unique.subscribe(prefix)
	}
}
