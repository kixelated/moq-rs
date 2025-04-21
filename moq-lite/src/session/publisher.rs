use std::collections::HashMap;

use crate::{message, model::GroupConsumer, Announced, AnnouncedProducer, Broadcast, BroadcastConsumer, Error, Track};

use web_async::{spawn, FuturesExt, Lock};

use super::{OrClose, Stream, Writer};

#[derive(Clone)]
pub(super) struct Publisher {
	session: web_transport::Session,
	announced: AnnouncedProducer,
	broadcasts: Lock<HashMap<Broadcast, BroadcastConsumer>>,
}

impl Publisher {
	pub fn new(session: web_transport::Session) -> Self {
		// We start the publisher in live mode because we're producing content.
		let announced = AnnouncedProducer::new();

		Self {
			session,
			announced,
			broadcasts: Default::default(),
		}
	}

	/// Publish a broadcast.
	pub fn publish(&mut self, broadcast: BroadcastConsumer) {
		// TODO properly handle duplicates
		self.broadcasts.lock().insert(broadcast.info.clone(), broadcast.clone());
		self.announced.announce(broadcast.info.clone());

		tracing::debug!(broadcast = %broadcast.info.path, "published");

		let mut this = self.clone();

		spawn(async move {
			tokio::select! {
				_ = broadcast.closed() => (),
				_ = this.session.closed() => (),
			}
			this.broadcasts.lock().remove(&broadcast.info);
			this.announced.unannounce(&broadcast.info);

			tracing::debug!(broadcast = %broadcast.info.path, "unpublished");
		});
	}

	pub async fn recv_announce(&mut self, stream: &mut Stream) -> Result<(), Error> {
		let interest = stream.reader.decode::<message::AnnounceRequest>().await?;
		let prefix = interest.prefix;
		tracing::debug!(%prefix, "serving announcements");

		let mut announced = self.announced.subscribe(&prefix);

		// Flush any synchronously announced paths
		while let Some(announced) = announced.next().await {
			match announced {
				Announced::Active(broadcast) => {
					tracing::debug!(broadcast = %broadcast.path, "served announce");
					let suffix = broadcast.strip_prefix(&prefix).ok_or(Error::ProtocolViolation)?;

					let msg = message::Announce::Active {
						suffix: suffix.path.to_string(),
					};
					stream.writer.encode(&msg).await?;
				}
				Announced::Ended(broadcast) => {
					tracing::debug!(broadcast = %broadcast.path, "served unannounced");
					let suffix = broadcast.strip_prefix(&prefix).ok_or(Error::ProtocolViolation)?;
					let msg = message::Announce::Ended {
						suffix: suffix.path.to_string(),
					};
					stream.writer.encode(&msg).await?;
				}
			}
		}

		Ok(())
	}

	pub async fn recv_subscribe(&mut self, stream: &mut Stream) -> Result<(), Error> {
		let subscribe = stream.reader.decode().await?;
		self.serve_subscribe(stream, subscribe).await
	}

	async fn serve_subscribe(&mut self, stream: &mut Stream, subscribe: message::Subscribe) -> Result<(), Error> {
		let track = Track {
			name: subscribe.track,
			priority: subscribe.priority,
		};

		let broadcast = Broadcast::new(subscribe.broadcast);

		let broadcast = self.broadcasts.lock().get(&broadcast).ok_or(Error::NotFound)?.clone();
		let mut track = broadcast.subscribe(track).await?;

		let info = message::SubscribeOk {
			priority: track.info.priority,
		};

		tracing::info!(broadcast = %broadcast.info.path, track = %track.info.name, id = subscribe.id, "serving subscription");

		stream.writer.encode(&info).await?;

		let mut complete = false;

		loop {
			tokio::select! {
				Some(group) = track.next_group().transpose() => {
					let mut group = group?;
					let session = self.session.clone();
					let priority = Self::stream_priority(subscribe.priority, group.info.sequence);

					spawn(async move {
						if let Err(err) = Self::serve_group(session, subscribe.id, priority, &mut group).await {
							tracing::warn!(?err, subscribe = ?subscribe.id, group = ?group.info, "dropped group");
						}
					});
				},
				res = stream.reader.decode_maybe::<message::SubscribeUpdate>(), if !complete => match res? {
					Some(_update) => {
						// TODO use it
					},
					// Subscribe has completed
					None => {
						complete = true;
					}
				},
				else => break,
			}
		}

		tracing::info!(broadcast = %broadcast.info.path, track = %track.info.name, id = subscribe.id, "subscription complete");

		Ok(())
	}

	pub async fn serve_group(
		mut session: web_transport::Session,
		subscribe: u64,
		priority: i32,
		group: &mut GroupConsumer,
	) -> Result<(), Error> {
		// TODO open streams in priority order to help with MAX_STREAMS flow control issues.
		let mut stream = Writer::open(&mut session, message::DataType::Group).await?;
		stream.set_priority(priority);

		Self::serve_group_inner(subscribe, group, &mut stream)
			.await
			.or_close(&mut stream)
	}

	pub async fn serve_group_inner(
		subscribe: u64,
		group: &mut GroupConsumer,
		stream: &mut Writer,
	) -> Result<(), Error> {
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

		// TODO block until all bytes have been acknowledged so we can still reset
		// writer.finish().await?;

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
