use crate::{message, model::GroupConsumer, Announced, Broadcast, BroadcastConsumer, Error, Origin, Track};
use web_async::FuturesExt;

use super::{Stream, Writer};

#[derive(Clone)]
pub(super) struct Publisher {
	session: web_transport::Session,
	broadcasts: Origin,
}

impl Publisher {
	pub fn new(session: web_transport::Session) -> Self {
		Self {
			session,
			broadcasts: Default::default(),
		}
	}

	/// Publish a broadcast, returning the previous instance if it already exists.
	pub fn publish(&mut self, broadcast: BroadcastConsumer) -> Option<BroadcastConsumer> {
		self.broadcasts.publish(broadcast)
	}

	pub async fn recv_announce(&mut self, mut stream: Stream) -> Result<(), Error> {
		let interest = stream.reader.decode::<message::AnnounceRequest>().await?;
		let prefix = interest.prefix;

		tracing::debug!(%prefix, "announce started");

		if let Err(err) = self.recv_announce_inner(stream, &prefix).await {
			tracing::warn!(?err, %prefix, "announce error");
		} else {
			tracing::debug!(%prefix, "announce complete");
		}

		Ok(())
	}

	async fn recv_announce_inner(&mut self, mut stream: Stream, prefix: &str) -> Result<(), Error> {
		let mut announced = self.broadcasts.announced(prefix);

		// Flush any synchronously announced paths
		loop {
			tokio::select! {
				announced = announced.next() => {
					match announced {
						Some(Announced::Start(broadcast)) => {
							let suffix = broadcast.path.strip_prefix(prefix).ok_or(Error::ProtocolViolation)?;

							let msg = message::Announce::Active {
								suffix: suffix.to_string(),
							};
							stream.writer.encode(&msg).await?;
						}
						Some(Announced::End(broadcast)) => {
							let suffix = broadcast.path.strip_prefix(prefix).ok_or(Error::ProtocolViolation)?;
							let msg = message::Announce::Ended {
								suffix: suffix.to_string(),
							};
							stream.writer.encode(&msg).await?;
						}
						None => break,
					}
				}
				res = stream.reader.finished() => return res,
			}
		}

		stream.writer.close();
		stream.reader.finished().await?;

		Ok(())
	}

	pub async fn recv_subscribe(&mut self, mut stream: Stream) -> Result<(), Error> {
		let subscribe = stream.reader.decode::<message::Subscribe>().await?;

		tracing::debug!(id = %subscribe.id, broadcast = %subscribe.broadcast, track = %subscribe.track, "subscribe started");

		if let Err(err) = self.recv_subscribe_inner(stream, &subscribe).await {
			tracing::warn!(?err, id = %subscribe.id, broadcast = %subscribe.broadcast, track = %subscribe.track, "subscribe error");
		} else {
			tracing::debug!(id = %subscribe.id, broadcast = %subscribe.broadcast, track = %subscribe.track, "subscribe complete");
		}

		Ok(())
	}

	async fn recv_subscribe_inner(&mut self, mut stream: Stream, subscribe: &message::Subscribe) -> Result<(), Error> {
		let broadcast = Broadcast::new(subscribe.broadcast.clone());
		let track = Track {
			name: subscribe.track.clone(),
			priority: subscribe.priority,
		};

		let broadcast = self.broadcasts.consume(&broadcast).ok_or(Error::NotFound)?;
		let mut track = broadcast.subscribe(&track);

		// TODO wait until track.info() to get the *real* priority

		let info = message::SubscribeOk {
			priority: track.info.priority,
		};

		stream.writer.encode(&info).await?;

		// Kind of hacky, but we only serve up to two groups concurrently.
		// This avoids a race where we try to cancel the previous group at the same time as we FIN it.
		// We don't want to allow N concurrent groups otherwise slow consumers will eat our RAM.
		// FuturesOrdered would be used if there was a `pop` method.
		let mut old_group = None;
		let mut new_group = None;

		loop {
			tokio::select! {
				Some(group) = track.next_group().transpose() => {
					let group = group?;
					let session = self.session.clone();
					let priority = Self::stream_priority(subscribe.priority, group.info.sequence);

					let future = Some(Box::pin(Self::serve_group(session, subscribe.id, priority, group)));
					if new_group.is_none() {
						new_group = future;
					} else {
						old_group = new_group;
						new_group = future;
					}
				},
				Some(res) = async { Some(old_group.as_mut()?.await) } => {
					old_group = None;
					if let Err(err) = res {
						tracing::warn!(?err, subscribe = ?subscribe.id, "group error");
					}
				},
				Some(res) = async { Some(new_group.as_mut()?.await) } => {
					new_group = None;
					old_group = None; // Also cancel the old group because it's so far behind.

					if let Err(err) = res {
						tracing::warn!(?err, subscribe = ?subscribe.id, "group error");
					}
				},
				res = stream.reader.decode_maybe::<message::SubscribeUpdate>() => match res? {
					Some(_update) => {
						// TODO use it
					},
					// Subscribe has completed
					None => break,
				},
			}
		}

		stream.writer.close();
		stream.reader.finished().await?;

		Ok(())
	}

	pub async fn serve_group(
		mut session: web_transport::Session,
		subscribe: u64,
		priority: i32,
		mut group: GroupConsumer,
	) -> Result<(), Error> {
		// TODO open streams in priority order to help with MAX_STREAMS flow control issues.
		let mut stream = Writer::open(&mut session, message::DataType::Group).await?;

		stream.set_priority(priority);

		let msg = message::Group {
			subscribe,
			sequence: group.info.sequence,
		};

		stream.encode(&msg).await?;

		while let Some(mut frame) = group.next_frame().await? {
			let header = message::Frame { size: frame.info.size };
			stream.encode(&header).await?;

			let mut remain = frame.info.size;

			while let Some(chunk) = frame.read().await? {
				remain = remain.checked_sub(chunk.len() as u64).ok_or(Error::WrongSize)?;
				stream.write(&chunk).await?;
			}

			if remain > 0 {
				return Err(Error::WrongSize);
			}
		}

		stream.close();

		Ok(())
	}

	// Quinn takes a i32 priority.
	// We do our best to distill 70 bits of information into 32 bits, but overflows will happen.
	// Specifically, group sequence 2^24 will overflow and be incorrectly prioritized.
	// But even with a group per frame, it will take ~6 days to reach that point.
	// TODO The behavior when two tracks share the same priority is undefined. Should we round-robin?
	fn stream_priority(track_priority: i8, group_sequence: u64) -> i32 {
		let sequence = (0xFFFFFF - group_sequence as u32) & 0xFFFFFF;
		((track_priority as i32) << 24) | sequence as i32
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn stream_priority() {
		let assert = |track_priority, group_sequence, expected| {
			assert_eq!(Publisher::stream_priority(track_priority, group_sequence), expected);
		};

		const U24: i32 = (1 << 24) - 1;

		// NOTE: The lower the value, the higher the priority.
		assert(-1, 50, -51);
		assert(-1, 0, -1);
		assert(0, 50, U24 - 50);
		assert(0, 0, U24);
		assert(1, 50, 2 * U24 - 49);
		assert(1, 0, 2 * U24 + 1);
	}
}
