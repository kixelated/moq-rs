use std::sync::{Arc, Mutex};

use bytes::{Buf, BufMut, BytesMut};
use moq_lite::{TrackConsumer, TrackProducer};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Position {
	// -1 to 1 is within the viewport, but we can go outside of that.
	pub x: f32,

	pub y: f32,

	// 0.25 to 4 is the zoom level, where 1 is 100%
	pub zoom: f32,
}

impl Default for Position {
	fn default() -> Self {
		Self {
			x: 0.0,
			y: 0.0,
			zoom: 1.0,
		}
	}
}

pub struct LocationConsumer {
	pub track: TrackConsumer,
}

impl LocationConsumer {
	pub fn new(track: TrackConsumer) -> Self {
		Self { track }
	}

	pub async fn next(&mut self) -> Result<Option<Position>> {
		loop {
			let mut group = match self.track.next_group().await? {
				Some(group) => group,
				None => return Ok(None),
			};

			let mut frame = match group.read_frame().await? {
				Some(frame) => frame,
				None => return Err(Error::EmptyGroup),
			};

			if frame.len() != 12 {
				return Err(Error::InvalidFrame);
			}

			let x = frame.get_f32();
			let y = frame.get_f32();
			let zoom = frame.get_f32();

			return Ok(Some(Position { x, y, zoom }));
		}
	}
}

#[derive(Clone)]
pub struct LocationProducer {
	pub track: TrackProducer,
	latest: Arc<Mutex<Option<Position>>>,
}

impl LocationProducer {
	pub fn new(track: TrackProducer) -> Self {
		Self {
			track,
			latest: Arc::new(Mutex::new(None)),
		}
	}

	pub fn latest(&self) -> Option<Position> {
		self.latest.lock().unwrap().clone()
	}

	pub fn update(&mut self, position: Position) {
		let mut group = self.track.append_group();

		// Encode the floats to the buffer.
		let mut buffer = BytesMut::with_capacity(12);
		buffer.put_f32(position.x);
		buffer.put_f32(position.y);
		buffer.put_f32(position.zoom);

		group.write_frame(buffer);
		group.finish();

		*self.latest.lock().unwrap() = Some(position);
	}

	// Given LocationConsumer, update our position to match.
	pub async fn reflect(&mut self, mut consumer: LocationConsumer) -> Result<()> {
		while let Some(position) = consumer.next().await? {
			self.update(position);
		}

		Ok(())
	}

	pub fn finish(self) {
		self.track.finish();
	}
}
