use crate::{Error, Frame, Timestamp};
use serde::{Deserialize, Serialize};

use moq_transfork::coding::*;

use derive_more::Debug;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Track {
	pub name: String,
	pub priority: i8,
}

#[derive(Debug)]
#[debug("{:?}", track.path)]
pub struct TrackProducer {
	track: moq_transfork::TrackProducer,
	group: Option<moq_transfork::GroupProducer>,
}

impl TrackProducer {
	pub fn new(track: moq_transfork::TrackProducer) -> Self {
		Self { track, group: None }
	}

	#[tracing::instrument("frame", skip_all, fields(track = ?self.track.path.last().unwrap()))]
	pub fn write(&mut self, frame: Frame) {
		let timestamp = frame.timestamp.as_micros();
		let mut header = BytesMut::with_capacity(timestamp.encode_size());
		timestamp.encode(&mut header);

		let mut group = match self.group.take() {
			Some(group) if !frame.keyframe => group,
			_ => self.track.append_group(),
		};

		if frame.keyframe {
			tracing::debug!(group = ?group.sequence, ?frame, "encoded keyframe");
		} else {
			tracing::trace!(group = ?group.sequence, index = ?group.frame_count(), ?frame, "encoded frame");
		}

		let mut chunked = group.create_frame(header.len() + frame.payload.len());
		chunked.write(header.freeze());
		chunked.write(frame.payload);

		self.group.replace(group);
	}
}

#[derive(Debug)]
#[debug("{:?}", track.path)]
pub struct TrackConsumer {
	track: moq_transfork::TrackConsumer,
	group: Option<moq_transfork::GroupConsumer>,
}

impl TrackConsumer {
	pub fn new(track: moq_transfork::TrackConsumer) -> Self {
		Self { track, group: None }
	}

	#[tracing::instrument("frame", skip_all, fields(track = ?self.track.path.last().unwrap()))]
	pub async fn read(&mut self) -> Result<Option<Frame>, Error> {
		loop {
			tokio::select! {
				biased;
				Some(res) = async { self.group.as_mut()?.read_frame().await.transpose() } => {
					let raw = res?;

					let index = self.group.as_ref().unwrap().frame_index() - 1;
					let keyframe = index == 0;
					let frame =  self.decode_frame(raw, keyframe)?;

					// Get some information about the group for logging
					let group = self.group.as_ref().unwrap();
					let group = group.sequence;

					if keyframe {
						tracing::debug!(?frame, ?group, "decoded keyframe");
					} else {
						tracing::trace!(?frame, ?group, ?index, "decoded frame");
					}

					return Ok(Some(frame));
				},
				Some(res) = async { self.track.next_group().await.transpose() } => {
					let group = res?;

					match &self.group {
						Some(existing) if group.sequence < existing.sequence => {
							// Ignore old groups
							continue;
						},
						// TODO use a configurable latency before moving to the next group.
						_ => self.group = Some(group),
					}
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
