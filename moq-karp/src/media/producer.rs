use moq_transfork::{coding::*, Path, Session};

use super::{Error, Timestamp};
use crate::catalog;

pub struct BroadcastProducer {
	catalog: catalog::Broadcast,
	catalog_track: Option<moq_transfork::TrackProducer>,
	session: Session,
	path: Path,
}

impl BroadcastProducer {
	pub fn new(session: Session, path: Path) -> Self {
		Self {
			session,
			path,
			catalog: catalog::Broadcast::default(),
			catalog_track: None,
		}
	}

	pub fn create_video(&mut self, info: catalog::Video) -> Result<TrackProducer, Error> {
		let (producer, consumer) = moq_transfork::Track {
			path: self.path.clone().push(&info.name),
			priority: info.priority,
			..Default::default()
		}
		.produce();

		self.session.publish(consumer)?;
		let track = TrackProducer::new(producer);

		self.catalog.video.push(info);
		Ok(track)
	}

	pub fn create_audio(&mut self, info: catalog::Audio) -> Result<TrackProducer, Error> {
		let (producer, consumer) = moq_transfork::Track {
			path: self.path.clone().push(&info.name),
			priority: info.priority,
			..Default::default()
		}
		.produce();

		self.session.publish(consumer)?;
		let track = TrackProducer::new(producer);

		self.catalog.audio.push(info);
		Ok(track)
	}

	pub fn catalog(&self) -> &catalog::Broadcast {
		&self.catalog
	}

	pub fn publish(&mut self) -> Result<(), Error> {
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

pub struct TrackProducer {
	inner: moq_transfork::TrackProducer,
	group: Option<moq_transfork::GroupProducer>,
}

impl TrackProducer {
	fn new(inner: moq_transfork::TrackProducer) -> Self {
		Self { inner, group: None }
	}

	pub fn keyframe(&mut self) {
		// The take() is important, it means we'll create a new group on the next write.
		if let Some(group) = self.group.take() {
			tracing::debug!(sequence = group.sequence, frames = group.frame_count(), "keyframe");
		}
	}

	pub fn write(&mut self, timestamp: Timestamp, payload: Bytes) {
		let timestamp = timestamp.as_micros();
		let mut header = BytesMut::with_capacity(timestamp.encode_size());
		timestamp.encode(&mut header);

		let mut group = match self.group.take() {
			Some(group) => group,
			None => self.inner.append_group(),
		};

		let mut frame = group.create_frame(header.len() + payload.len());
		frame.write(header.freeze());
		frame.write(payload);

		self.group.replace(group);
	}
}
