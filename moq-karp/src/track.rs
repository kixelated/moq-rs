use std::collections::VecDeque;

use crate::{Error, Frame, GroupConsumer, Timestamp};
use futures::{stream::FuturesUnordered, StreamExt};
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
		let timestamp = frame.timestamp.as_micros() as u64;
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

	pub fn subscribe(&self) -> TrackConsumer {
		TrackConsumer::new(self.track.subscribe())
	}
}

#[derive(Debug)]
#[debug("{:?}", track.path)]
pub struct TrackConsumer {
	track: moq_transfork::TrackConsumer,

	// The current group that we are reading from.
	current: Option<GroupConsumer>,

	// Future groups that we are monitoring, deciding based on [latency] whether to skip.
	pending: VecDeque<GroupConsumer>,

	// The maximum timestamp seen thus far, or zero because that's easier than None.
	max_timestamp: Timestamp,

	// The maximum buffer size before skipping a group.
	latency: std::time::Duration,
}

impl TrackConsumer {
	pub fn new(track: moq_transfork::TrackConsumer) -> Self {
		Self {
			track,
			current: None,
			pending: VecDeque::new(),
			max_timestamp: Timestamp::default(),
			latency: std::time::Duration::ZERO,
		}
	}

	#[tracing::instrument("frame", skip_all, fields(track = ?self.track.path.last().unwrap()))]
	pub async fn read(&mut self) -> Result<Option<Frame>, Error> {
		loop {
			let cutoff = self.max_timestamp + self.latency;

			// Keep track of all pending groups, buffering until we detect a timestamp far enough in the future.
			// This is a race; only the first group will succeed.
			// TODO is there a way to do this without FuturesUnordered?
			let mut buffering = FuturesUnordered::new();
			for (index, pending) in self.pending.iter_mut().enumerate() {
				buffering.push(async move { (index, pending.buffer_frames_until(cutoff).await) })
			}

			tokio::select! {
				biased;
				Some(res) = async { Some(self.current.as_mut()?.read_frame().await) } => {
					drop(buffering);

					match res? {
						// Got the next frame.
						Some(frame) => return Ok(Some(frame)),
						None => {
							// Group ended cleanly, instantly move to the next group.
							self.current = self.pending.pop_front();
							continue;
						}
					};
				},
				Some(res) = async { self.track.next_group().await.transpose() } => {
					let group = GroupConsumer::new(res?);
					drop(buffering);

					match self.current.as_ref() {
						Some(current) if group.sequence < current.sequence => {
							// Ignore old groups
							tracing::debug!(old = ?group.sequence, current = ?current.sequence, "skipping old group");
						},
						Some(_) => {
							// Insert into pending based on the sequence number ascending.
							let index = self.pending.partition_point(|g| g.sequence < group.sequence);
							self.pending.insert(index, group);
						},
						None => self.current = Some(group),
					};
				},
				Some((index, timestamp)) = buffering.next() => {
					tracing::debug!(old = ?self.max_timestamp, new = ?timestamp, buffer = ?self.latency, "skipping slow group");
					drop(buffering);

					if index > 0 {
						self.pending.drain(0..index);
						tracing::debug!(count = index, "skipping additional groups");
					}

					self.current = self.pending.pop_front();
				}
				else => return Ok(None),
			}
		}
	}

	pub fn set_latency(&mut self, max: std::time::Duration) {
		self.latency = max;
	}

	pub async fn closed(&self) -> Result<(), Error> {
		self.track.closed().await.map_err(Into::into)
	}
}
