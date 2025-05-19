use std::collections::HashSet;

use moq_lite::Broadcast;
use web_async::Lock;

use crate::{BroadcastConsumer, BroadcastProducer};

#[derive(Clone)]
pub struct Room {
	pub path: String,
	announced: moq_lite::AnnouncedConsumer,
	session: moq_lite::Session,
	ourselves: Lock<HashSet<Broadcast>>,
}

pub enum Action {
	Join(String),
	Leave(String),
}

impl Room {
	pub fn new(session: moq_lite::Session, path: String) -> Self {
		Self {
			announced: session.announced(format!("{}/", path)),
			path,
			session,
			ourselves: Lock::new(HashSet::new()),
		}
	}

	// Joins the room and returns a producer for the broadcast.
	pub fn join(&mut self, name: String) -> BroadcastProducer {
		let broadcast = Broadcast::new(format!("{}/{}.hang", self.path, name));
		let ourselves = self.ourselves.clone();

		ourselves.lock().insert(broadcast.clone());

		let producer = broadcast.clone().produce();
		self.session.publish(producer.consume());

		let consumer = producer.consume();

		web_async::spawn(async move {
			consumer.closed().await;
			ourselves.lock().remove(&broadcast);
		});

		BroadcastProducer::new(producer)
	}

	/// Returns the next room action.
	pub async fn update(&mut self) -> Option<Action> {
		loop {
			let announced = self.announced.next().await?;
			if self.ourselves.lock().contains(announced.broadcast()) {
				continue;
			}

			let name = announced.path().strip_prefix(&self.path).unwrap().to_string();

			return match announced {
				moq_lite::Announced::Start(_) => Some(Action::Join(name)),
				moq_lite::Announced::End(_) => Some(Action::Leave(name)),
			};
		}
	}

	// Takes the name of the broadcast to watch within the room.
	pub fn watch(&mut self, name: &str) -> BroadcastConsumer {
		let broadcast = format!("{}/{}.hang", self.path, name);
		let consumer = self.session.consume(&broadcast.into());
		BroadcastConsumer::new(consumer)
	}
}
