use crate::{BroadcastAnnounced, BroadcastProducer, Result};

use derive_more::Debug;
use moq_transfork::Path;

#[derive(Debug)]
#[debug("{:?}", name)]
pub struct Room {
	pub session: moq_transfork::Session,
	pub name: String,
}

impl Room {
	pub fn new(session: moq_transfork::Session, name: String) -> Self {
		Self { session, name }
	}

	pub fn publish(&self, name: &str) -> Result<BroadcastProducer> {
		let path = Path::new().push(&self.name).push(name);
		BroadcastProducer::new(self.session.clone(), path)
	}

	/// Watch a broadcast with a given name.
	/// The returned [BroadcastAnnounced] will be updated as new broadcasts are announced.
	/// This allows viewers to automatically reconnect to the new broadcast ID if the producer crashes.
	pub fn watch(&self, name: &str) -> BroadcastAnnounced {
		let path = Path::new().push(&self.name).push(name);
		BroadcastAnnounced::new(self.session.clone(), path)
	}
}
