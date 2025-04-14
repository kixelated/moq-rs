use std::collections::HashSet;

use moq_lite::Broadcast;
use web_async::Lock;

use crate::{BroadcastConsumer, BroadcastProducer, Result};

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

	pub fn join(&mut self, name: String) -> Result<BroadcastProducer> {
		let broadcast = Broadcast::new(format!("{}/{}", self.path, name));
		let ourselves = self.ourselves.clone();

		ourselves.lock().insert(broadcast.clone());

		let producer = broadcast.clone().produce();
		self.session.publish(producer.consume())?;

		let consumer = producer.consume();

		web_async::spawn(async move {
			consumer.closed().await;
			ourselves.lock().remove(&broadcast);
		});

		Ok(producer.into())
	}

	pub async fn joined(&mut self) -> Option<BroadcastConsumer> {
		loop {
			let broadcast = self.announced.active().await?;
			if self.ourselves.lock().contains(&broadcast) {
				continue;
			}

			let consumer = self.session.subscribe(broadcast);
			return Some(consumer.into());
		}
	}
}
