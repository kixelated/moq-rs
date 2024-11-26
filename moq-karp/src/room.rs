use crate::{BroadcastConsumer, BroadcastProducer, Result};

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
	///
	/// This supports automatically reloading the catalog on publisher crash.
	pub fn watch(&self, name: &str) -> BroadcastConsumer {
		let path = Path::new().push(&self.name).push(name);
		BroadcastConsumer::new(self.session.clone(), path)
	}
}
