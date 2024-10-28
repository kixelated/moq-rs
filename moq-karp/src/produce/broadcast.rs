use moq_transfork::{Path, Session};

use crate::{
	catalog::{self},
	Error,
};

use super::{Audio, Video};

pub struct Broadcast {
	catalog: catalog::Broadcast,
	catalog_track: Option<moq_transfork::TrackProducer>,
	session: Session,
	path: Path,
}

impl Broadcast {
	pub fn new(session: Session, path: Path) -> Self {
		Self {
			session,
			path,
			catalog: catalog::Broadcast::default(),
			catalog_track: None,
		}
	}

	pub fn create_video(&mut self, info: catalog::Video) -> Result<Video, Error> {
		let (producer, consumer) = moq_transfork::Track {
			path: self.path.clone().push(&info.track.name),
			priority: info.track.priority,
			..Default::default()
		}
		.produce();

		self.session.publish(consumer)?;
		let track = Video::new(producer);

		self.catalog.video.push(info);
		self.publish()?;

		Ok(track)
	}

	pub fn create_audio(&mut self, info: catalog::Audio) -> Result<Audio, Error> {
		let (producer, consumer) = moq_transfork::Track {
			path: self.path.clone().push(&info.track.name),
			priority: info.track.priority,
			..Default::default()
		}
		.produce();

		self.session.publish(consumer)?;
		let track = Audio::new(producer);

		self.catalog.audio.push(info);
		self.publish()?;

		Ok(track)
	}

	pub fn catalog(&self) -> &catalog::Broadcast {
		&self.catalog
	}

	fn publish(&mut self) -> Result<(), Error> {
		if let Some(track) = self.catalog_track.as_mut() {
			return Ok(self.catalog.update(track)?);
		}

		let path = self.path.clone().push("catalog.json");
		self.catalog_track = self.catalog.publish(&mut self.session, path)?.into();

		Ok(())
	}

	pub async fn closed(&self) {
		self.session.closed().await;
	}
}
