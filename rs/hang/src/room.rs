use std::collections::HashSet;

use web_async::Lock;

use crate::{BroadcastConsumer, BroadcastProducer};

#[derive(Clone)]
pub struct Room {
	pub path: String,
	broadcasts: moq_lite::OriginConsumer,
	session: moq_lite::Session,
	ourselves: Lock<HashSet<String>>,
}

impl Room {
	pub fn new(session: moq_lite::Session, path: String) -> Self {
		Self {
			broadcasts: session.consume_prefix(&path),
			path,
			session,
			ourselves: Lock::new(HashSet::new()),
		}
	}

	// Joins the room and returns a producer for the broadcast.
	pub fn join(&mut self, name: String) -> BroadcastProducer {
		let broadcast = format!("{}{}", self.path, name);
		let ourselves = self.ourselves.clone();
		ourselves.lock().insert(broadcast.clone());

		let producer = BroadcastProducer::new();
		self.session.publish(broadcast, producer.inner.consume());

		let consumer = producer.consume();

		web_async::spawn(async move {
			consumer.closed().await;
			ourselves.lock().remove(&name);
		});

		producer
	}

	/// Returns the next broadcaster in the room (not including ourselves).
	pub async fn watch(&mut self) -> Option<BroadcastConsumer> {
		loop {
			let (prefix, broadcast) = self.broadcasts.next().await?;
			if self.ourselves.lock().contains(&prefix) {
				continue;
			}

			return Some(BroadcastConsumer::new(broadcast));
		}
	}
}
