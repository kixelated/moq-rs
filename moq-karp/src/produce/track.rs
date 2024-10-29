use moq_transfork::coding::*;

use crate::media::Frame;

pub struct Track {
	inner: moq_transfork::TrackProducer,
	group: Option<moq_transfork::GroupProducer>,
}

impl Track {
	pub(super) fn new(inner: moq_transfork::TrackProducer) -> Self {
		Self { inner, group: None }
	}

	pub fn write(&mut self, frame: Frame) {
		let timestamp = frame.timestamp.as_micros();
		let mut header = BytesMut::with_capacity(timestamp.encode_size());
		timestamp.encode(&mut header);

		let mut group = match self.group.take() {
			Some(group) if !frame.keyframe => group,
			_ => self.inner.append_group(),
		};

		let mut chunked = group.create_frame(header.len() + frame.payload.len());
		chunked.write(header.freeze());
		chunked.write(frame.payload);

		self.group.replace(group);
	}
}
