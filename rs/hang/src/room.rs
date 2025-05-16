use std::collections::HashSet;

use moq_lite::{Announced, Broadcast};
use web_async::Lock;

use crate::{BroadcastConsumer, BroadcastProducer};

#[derive(Clone)]
pub struct Room {
	pub path: String,
	announced: moq_lite::AnnouncedConsumer,
	session: moq_lite::Session,
	ourselves: Lock<HashSet<Broadcast>>,
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

	pub fn join(&mut self, name: String) -> BroadcastProducer {
		let broadcast = Broadcast::new(format!("{}/{}", self.path, name));
		let ourselves = self.ourselves.clone();

		ourselves.lock().insert(broadcast.clone());

		let producer = broadcast.clone().produce();
		self.session.publish(producer.consume());

		let consumer = producer.consume();

		web_async::spawn(async move {
			consumer.closed().await;
			ourselves.lock().remove(&broadcast);
		});

		producer.into()
	}

	pub async fn update(&mut self) -> Option<Announced> {
		loop {
			let announced = self.announced.next().await?;
			if self.ourselves.lock().contains(announced.broadcast()) {
				continue;
			}

			return Some(announced);
		}
	}

	pub fn watch(&mut self, broadcast: &Broadcast) -> BroadcastConsumer {
		let consumer = self.session.consume(broadcast);
		consumer.into()
	}
}
