use crate::{BroadcastAnnounced, BroadcastProducer, Result};

use derive_more::Debug;

#[derive(Debug)]
#[debug("{:?}", path)]
pub struct Room {
	pub session: moq_transfork::Session,
	pub path: moq_transfork::Path,
}

impl Room {
	pub fn new(session: moq_transfork::Session, path: moq_transfork::Path) -> Self {
		Self { session, path }
	}

	pub fn publish(&self, name: String) -> Result<BroadcastProducer> {
		// Generate a "unique" ID for this broadcast session.
		// If we crash, then the viewers will automatically reconnect to the new ID.
		let id = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_millis();

		let path = self.path.clone().push(name).push(id);
		BroadcastProducer::new(self.session.clone(), path)
	}

	/// Watch a broadcast with a given name.
	/// The returned [BroadcastAnnounced] will be updated as new broadcasts are announced.
	/// This allows viewers to automatically reconnect to the new broadcast ID if the producer crashes.
	pub fn watch(&self, name: String) -> BroadcastAnnounced {
		let path = self.path.clone().push(name);
		BroadcastAnnounced::new(self.session.clone(), path)
	}
}
