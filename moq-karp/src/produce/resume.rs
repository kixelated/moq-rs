use std::time;

use moq_transfork::{Path, Session};

use super::Broadcast;

pub struct Resumable {
	session: Session,
	path: Path,
}

impl Resumable {
	pub fn new(session: Session, path: Path) -> Self {
		Self { session, path }
	}

	/// Produce a broadcast using the current time as the ID.
	/// This can be used across restarts to resume a broadcast... provided you don't reset your clock.
	pub fn broadcast(&self) -> Broadcast {
		let id = time::SystemTime::now()
			.duration_since(time::UNIX_EPOCH)
			.unwrap()
			.as_secs();

		let path = self.path.clone().push(id.to_string());
		Broadcast::new(self.session.clone(), path)
	}
}
