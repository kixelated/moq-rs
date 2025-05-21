use std::collections::HashSet;

use web_async::Lock;

use crate::{BroadcastConsumer, BroadcastProducer};

#[derive(Clone)]
pub struct Room {
	pub path: String,
	announced: moq_lite::AnnounceConsumer,
	session: moq_lite::Session,
	ourselves: Lock<HashSet<String>>,
}

impl Room {
	pub fn new(session: moq_lite::Session, path: String) -> Self {
		Self {
			announced: session.announced(path.clone()),
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

	/// Returns the next room action.
	pub async fn update(&mut self) -> Option<moq_lite::Announce> {
		loop {
			let announced = self.announced.next().await?;
			if self.ourselves.lock().contains(announced.suffix()) {
				continue;
			}

			return Some(announced);
		}
	}

	// Takes the name of the broadcast to watch within the room.
	pub fn watch(&mut self, name: &str) -> BroadcastConsumer {
		let path = format!("{}{}", self.path, name);
		let broadcast = self.session.consume(&path);
		BroadcastConsumer::new(broadcast)
	}
}
