use moq_transfork::coding::*;

use super::Error;
use crate::catalog;

pub struct BroadcastProducer {
	catalog: catalog::Broadcast,
	catalog_track: Option<moq_transfork::TrackProducer>,
	inner: moq_transfork::BroadcastProducer,
}

impl BroadcastProducer {
	pub fn new(broadcast: moq_transfork::BroadcastProducer) -> Self {
		Self {
			inner: broadcast,
			catalog: catalog::Broadcast::default(),
			catalog_track: None,
		}
	}

	pub fn create_video(&mut self, info: catalog::Video) -> Result<TrackProducer, Error> {
		let track = info.track.clone();
		if self.inner.has_track(&track.name) {
			return Err(Error::DuplicateTrack);
		}

		let track = self.inner.insert_track(track);
		let track = TrackProducer::new(track);

		self.catalog.video.push(info);
		Ok(track)
	}

	pub fn create_audio(&mut self, info: catalog::Audio) -> Result<TrackProducer, Error> {
		let track = info.track.clone();
		if self.inner.has_track(&track.name) {
			return Err(Error::DuplicateTrack);
		}

		let track = self.inner.insert_track(track);
		let track = TrackProducer::new(track);

		self.catalog.audio.push(info);
		Ok(track)
	}

	pub fn catalog(&self) -> &catalog::Broadcast {
		&self.catalog
	}

	pub fn publish(&mut self) -> Result<(), Error> {
		match self.catalog_track.as_mut() {
			Some(track) => self.catalog.update(track)?,
			None => self.catalog_track = self.catalog.publish(&mut self.inner)?.into(),
		};

		Ok(())
	}

	pub async fn closed(&self) {
		self.inner.closed().await
	}

	pub fn is_closed(&self) -> bool {
		self.inner.is_closed()
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

	pub fn write(&mut self, timestamp: u64, payload: Bytes) {
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
