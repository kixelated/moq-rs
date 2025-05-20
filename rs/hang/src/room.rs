use std::collections::HashSet;

use web_async::Lock;

use crate::{Broadcast, BroadcastConsumer, BroadcastProducer};

#[derive(Clone)]
pub struct Room {
	pub path: String,
	announced: moq_lite::AnnouncedConsumer,
	session: moq_lite::Session,
	ourselves: Lock<HashSet<String>>,
}

pub enum Action {
	Join(String),
	Leave(String),
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
		let broadcast = Broadcast {
			path: format!("{}{}", self.path, name),
		};

		let ourselves = self.ourselves.clone();

		ourselves.lock().insert(name.clone());

		let producer = broadcast.produce();
		self.session.publish(producer.consume());

		let consumer = producer.consume();

		web_async::spawn(async move {
			consumer.closed().await;
			ourselves.lock().remove(&name);
		});

		producer.into()
	}

	/// Returns the next room action.
	pub async fn update(&mut self) -> Option<Action> {
		loop {
			let announced = self.announced.next().await?;
			let name = announced.path().strip_prefix(&self.path).unwrap().to_string();
			if self.ourselves.lock().contains(&name) {
				continue;
			}

			return match announced {
				moq_lite::Announced::Start(_) => Some(Action::Join(name)),
				moq_lite::Announced::End(_) => Some(Action::Leave(name)),
			};
		}
	}

	// Takes the name of the broadcast to watch within the room.
	pub fn watch(&mut self, name: &str) -> BroadcastConsumer {
		let info = Broadcast {
			path: format!("{}{}", self.path, name),
		};

		let broadcast = self.session.consume(&info);
		broadcast.into()
	}
}
