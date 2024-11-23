use std::collections::HashSet;

use crate::{
	broadcast::{Broadcast, BroadcastConsumer, BroadcastProducer},
	Error, Result,
};

use moq_transfork::{Announced, AnnouncedConsumer, Path, Session};

#[derive(Clone)]
pub struct Room {
	pub session: Session,
	pub path: Path,
}

impl Room {
	pub fn new(session: Session, path: Path) -> Self {
		Self { session, path }
	}

	pub fn produce(self) -> RoomProducer {
		RoomProducer::new(self)
	}

	pub fn consume(self) -> RoomConsumer {
		RoomConsumer::new(self)
	}
}

pub struct RoomProducer {
	room: Room,
}

impl RoomProducer {
	pub fn new(room: Room) -> Self {
		Self { room }
	}

	/// Produce a broadcast using the current time as the ID.
	/// This can be used across restarts to resume a broadcast... provided you don't reset your clock.
	pub fn broadcast<T: ToString>(&self, name: T) -> Result<BroadcastProducer> {
		Broadcast {
			session: self.session.clone(),
			path: self.path.clone().push(name.to_string()),
		}
		.produce()
	}

	// Use the current time as the broadcast ID.
	pub fn broadcast_now(&self) -> Result<BroadcastProducer> {
		let id = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_millis();
		self.broadcast(id)
	}
}

impl std::ops::Deref for RoomProducer {
	type Target = Room;

	fn deref(&self) -> &Self::Target {
		&self.room
	}
}

/// Provides resumable access to broadcasts.
/// Each broadcast is identified by an increasing ID, allowing the consumer to discover crashes and use the higher ID.
pub struct RoomConsumer {
	room: Room,

	announced: AnnouncedConsumer,
	active: HashSet<String>,
}

impl RoomConsumer {
	pub fn new(room: Room) -> Self {
		let announced = room.session.announced_prefix(room.path.clone());

		Self {
			room,
			announced,
			active: HashSet::new(),
		}
	}

	// Returns the next unique broadcast.
	pub async fn broadcast(&mut self) -> Option<BroadcastConsumer> {
		while let Some(broadcast) = self.announced.next().await {
			match broadcast {
				Announced::Active(path) => match self.try_load(path).await {
					Ok(consumer) => return consumer,
					Err(err) => tracing::warn!(?err, "failed to load broadcast"),
				},
				Announced::Ended(path) => self.unload(path),
			}
		}

		None
	}

	async fn try_load(&mut self, path: Path) -> Result<Option<BroadcastConsumer>> {
		let suffix = path.strip_prefix(self.announced.prefix()).unwrap();
		let name = suffix.first().ok_or(Error::InvalidSession)?;

		if self.active.contains(name.as_str()) {
			// Skip duplicates
			return Ok(None);
		}

		let path = self.announced.prefix().clone().push(name);

		let broadcast = Broadcast {
			path,
			session: self.room.session.clone(),
		}
		.consume();

		self.active.insert(name.to_string());

		Ok(Some(broadcast))
	}

	fn unload(&mut self, path: Path) {
		let suffix = path.strip_prefix(self.announced.prefix()).unwrap();
		let name = suffix.first().expect("invalid path");
		self.active.remove(name.as_str());
	}
}

impl std::ops::Deref for RoomConsumer {
	type Target = Room;

	fn deref(&self) -> &Self::Target {
		&self.room
	}
}
