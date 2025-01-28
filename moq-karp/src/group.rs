use std::collections::VecDeque;

use crate::{Frame, Result, Timestamp};
use moq_transfork::coding::Decode;

#[derive(Debug)]
pub struct GroupConsumer {
	// The MoqTransfork group (no timestamp information)
	group: moq_transfork::GroupConsumer,

	// The current frame index
	index: usize,

	// The any buffered frames in the group.
	buffered: VecDeque<Frame>,

	// The max timestamp in the group
	max_timestamp: Option<Timestamp>,
}

impl GroupConsumer {
	pub fn new(group: moq_transfork::GroupConsumer) -> Self {
		Self {
			group,
			index: 0,
			buffered: VecDeque::new(),
			max_timestamp: None,
		}
	}

	pub async fn read_frame(&mut self) -> Result<Option<Frame>> {
		if let Some(frame) = self.buffered.pop_front() {
			Ok(Some(frame))
		} else {
			self.read_frame_unbuffered().await
		}
	}

	async fn read_frame_unbuffered(&mut self) -> Result<Option<Frame>> {
		let mut payload = match self.group.read_frame().await? {
			Some(payload) => payload,
			None => return Ok(None),
		};

		let micros = u64::decode(&mut payload)?;
		let timestamp = Timestamp::from_micros(micros);

		let frame = Frame {
			keyframe: (self.index == 0),
			timestamp,
			payload,
		};

		if frame.keyframe {
			tracing::debug!(?frame, group = ?self.group, "decoded keyframe");
		} else {
			tracing::trace!(?frame, group = ?self.group, index = self.index, "decoded frame");
		}

		self.index += 1;
		self.max_timestamp = Some(self.max_timestamp.unwrap_or_default().max(timestamp));

		Ok(Some(frame))
	}

	// Keep reading and buffering new frames, returning when `max` is larger than or equal to the cutoff.
	// Not publish because the API is super weird.
	// This will BLOCK FOREVER if the group has ended early; it's intended to be used within select!
	pub(super) async fn buffer_frames_until(&mut self, cutoff: Timestamp) -> Timestamp {
		loop {
			match self.max_timestamp {
				Some(timestamp) if timestamp >= cutoff => return timestamp,
				_ => (),
			}

			match self.read_frame().await {
				Ok(Some(frame)) => self.buffered.push_back(frame),
				// Otherwise block forever so we don't return from FuturesUnordered
				_ => std::future::pending().await,
			}
		}
	}

	pub fn max_timestamp(&self) -> Option<Timestamp> {
		self.max_timestamp
	}
}

impl std::ops::Deref for GroupConsumer {
	type Target = moq_transfork::GroupConsumer;

	fn deref(&self) -> &Self::Target {
		&self.group
	}
}
