use moq_transfork::coding::*;

use crate::media::Timestamp;

pub struct Audio {
	inner: moq_transfork::TrackProducer,
	group: Option<moq_transfork::GroupProducer>,
}

impl Audio {
	pub(super) fn new(inner: moq_transfork::TrackProducer) -> Self {
		Self { inner, group: None }
	}

	// Terminate an audio segment.
	// This should be done at least once per video keyframe, but may be done more frequently.
	pub fn segment(&mut self) {
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
