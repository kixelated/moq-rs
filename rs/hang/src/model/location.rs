use moq_lite::TrackProducer;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Location {
	// -1 to 1 is within the viewport, but we can go outside of that.
	pub x: f32,
	pub y: f32,
}

pub struct LocationProducer {
	track: TrackProducer,
}

impl LocationProducer {
	pub fn new(track: TrackProducer) -> Self {
		Self { track }
	}

	// If the handle is 0, then this is our own location.
	pub fn update(&mut self, handle: u32, location: Location) {
		let mut group = self.track.append_group();

		// Encode the two floats to the buffer.
		let mut buffer = Vec::new();
		// TODO save some bits and use a varint
		buffer.extend_from_slice(&handle.to_le_bytes());
		buffer.extend_from_slice(&location.x.to_le_bytes());
		buffer.extend_from_slice(&location.y.to_le_bytes());

		group.write_frame(buffer);
		group.finish();
	}

	pub fn finish(&mut self) {
		self.track.finish();
	}
}
