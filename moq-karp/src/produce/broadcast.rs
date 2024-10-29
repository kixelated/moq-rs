use moq_transfork::{Path, Session, TrackConsumer, TrackProducer};

use crate::{
	catalog::{self},
	Error,
};

use super::Track;

pub struct Broadcast {
	pub path: Path,
	catalog: catalog::Broadcast,
	catalog_producer: TrackProducer, // need to hold the track to keep it open
	tracks: Vec<TrackConsumer>,
}

impl Broadcast {
	pub fn new(path: Path) -> Self {
		let (producer, consumer) = catalog::Broadcast::track(path.clone()).produce();

		Self {
			path,
			catalog: catalog::Broadcast::default(),
			catalog_producer: producer,
			tracks: vec![consumer],
		}
	}

	pub fn create_video(&mut self, info: catalog::Video) -> Result<Track, Error> {
		let (producer, consumer) = moq_transfork::Track {
			path: self.path.clone().push(&info.track.name),
			priority: info.track.priority,
			..Default::default()
		}
		.produce();

		let track = Track::new(producer);

		self.catalog.video.push(info);
		self.tracks.push(consumer);

		self.update()?;

		Ok(track)
	}

	pub fn create_audio(&mut self, info: catalog::Audio) -> Result<Track, Error> {
		let (producer, consumer) = moq_transfork::Track {
			path: self.path.clone().push(&info.track.name),
			priority: info.track.priority,
			..Default::default()
		}
		.produce();

		let track = Track::new(producer);

		self.catalog.audio.push(info);
		self.tracks.push(consumer);

		self.update()?;

		Ok(track)
	}

	pub fn catalog(&self) -> &catalog::Broadcast {
		&self.catalog
	}

	fn update(&mut self) -> Result<(), Error> {
		let frame = self.catalog.to_string()?;

		let mut group = self.catalog_producer.append_group();
		group.write_frame(frame);

		Ok(())
	}

	/// Publish all of the *current* tracks to the session.
	pub fn publish(&self, session: &mut Session) -> Result<(), Error> {
		for track in &self.tracks {
			session.publish(track.clone())?;
		}

		Ok(())
	}
}
