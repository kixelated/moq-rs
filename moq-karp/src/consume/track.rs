use crate::media::Timestamp;
use crate::Error;

use crate::{media::Frame, util::FuturesExt};

use moq_transfork::coding::*;

pub struct Track {
	track: moq_transfork::TrackConsumer,
	group: Option<moq_transfork::GroupConsumer>,
}

impl Track {
	pub(super) fn new(track: moq_transfork::TrackConsumer) -> Self {
		Self { track, group: None }
	}

	pub async fn read(&mut self) -> Result<Option<Frame>, Error> {
		let mut keyframe = false;

		if self.group.is_none() {
			self.group = self.track.next_group().await?;
			keyframe = true;

			if self.group.is_none() {
				return Ok(None);
			}
		}

		loop {
			tokio::select! {
				biased;
				Some(res) = self.group.as_mut().unwrap().read_frame().transpose() => {
					let raw = res?;

					let frame =  self.decode_frame(raw, keyframe)?;

					// Get some information about the group for logging
					let group = self.group.as_ref().unwrap();
					let index = group.frame_index() - 1;
					let group = group.sequence;

					if keyframe {
						tracing::debug!(?frame, ?group, "decoded keyframe");
					} else {
						tracing::trace!(?frame, ?group, ?index, "decoded frame");
					}

					return Ok(Some(frame));
				},
				Some(res) = self.track.next_group().transpose() => {
					let group = res?;

					if group.sequence < self.group.as_ref().unwrap().sequence {
						// Ignore old groups
						continue;
					}

					// TODO use a configurable latency before moving to the next group.
					self.group = Some(group);
					keyframe = true;
				},
				else => return Ok(None),
			}
		}
	}

	fn decode_frame(&self, mut payload: Bytes, keyframe: bool) -> Result<Frame, Error> {
		let micros = u64::decode(&mut payload)?;
		let timestamp = Timestamp::from_micros(micros);

		let frame = Frame {
			keyframe,
			timestamp,
			payload,
		};

		Ok(frame)
	}
}
