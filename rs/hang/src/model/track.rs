use std::collections::VecDeque;

use crate::model::{Frame, GroupConsumer, Timestamp};
use crate::Error;
use futures::{stream::FuturesUnordered, StreamExt};

use moq_lite::coding::*;

#[derive(Clone)]
pub struct TrackProducer {
	pub inner: moq_lite::TrackProducer,
	group: Option<moq_lite::GroupProducer>,
}

impl TrackProducer {
	pub fn new(inner: moq_lite::TrackProducer) -> Self {
		Self { inner, group: None }
	}

	pub fn write(&mut self, frame: Frame) {
		let timestamp = frame.timestamp.as_micros() as u64;
		let mut header = BytesMut::with_capacity(timestamp.encode_size());
		timestamp.encode(&mut header);

		if frame.keyframe {
			if let Some(group) = self.group.take() {
				group.finish();
			}
		}

		let mut group = match self.group.take() {
			Some(group) => group,
			None => self.inner.append_group(),
		};

		let size = header.len() + frame.payload.len();
		let mut chunked = group.create_frame(size.into());
		chunked.write(header.freeze());
		chunked.write(frame.payload);
		chunked.finish();

		self.group.replace(group);
	}

	pub fn consume(&self) -> TrackConsumer {
		TrackConsumer::new(self.inner.consume())
	}
}

impl From<moq_lite::TrackProducer> for TrackProducer {
	fn from(inner: moq_lite::TrackProducer) -> Self {
		Self::new(inner)
	}
}

pub struct TrackConsumer {
	pub inner: moq_lite::TrackConsumer,

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
	pub fn new(inner: moq_lite::TrackConsumer) -> Self {
		Self {
			inner,
			current: None,
			pending: VecDeque::new(),
			max_timestamp: Timestamp::default(),
			latency: std::time::Duration::ZERO,
		}
	}

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
						Some(frame) => {
							self.max_timestamp = frame.timestamp;
							return Ok(Some(frame));
						}
						None => {
							// Group ended cleanly, instantly move to the next group.
							self.current = self.pending.pop_front();
							continue;
						}
					};
				},
				Some(res) = async { self.inner.next_group().await.transpose() } => {
					let group = GroupConsumer::new(res?);
					drop(buffering);

					match self.current.as_ref() {
						Some(current) if group.info.sequence < current.info.sequence => {
							// Ignore old groups
							tracing::debug!(old = ?group.info.sequence, current = ?current.info.sequence, "skipping old group");
						},
						Some(_) => {
							// Insert into pending based on the sequence number ascending.
							let index = self.pending.partition_point(|g| g.info.sequence < group.info.sequence);
							self.pending.insert(index, group);
						},
						None => self.current = Some(group),
					};
				},
				Some((index, timestamp)) = buffering.next() => {
					if self.current.is_some() {
						tracing::debug!(old = ?self.max_timestamp, new = ?timestamp, buffer = ?self.latency, "skipping slow group");
					}

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
		Ok(self.inner.closed().await?)
	}
}

impl From<moq_lite::TrackConsumer> for TrackConsumer {
	fn from(inner: moq_lite::TrackConsumer) -> Self {
		Self::new(inner)
	}
}
