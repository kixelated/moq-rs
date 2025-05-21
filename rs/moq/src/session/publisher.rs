use web_async::FuturesExt;

use crate::{message, model::GroupConsumer, BroadcastConsumer, Error, Origin, Track, TrackConsumer};

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

	/// Publish a broadcast.
	pub fn publish<T: ToString>(&mut self, path: T, broadcast: BroadcastConsumer) {
		self.broadcasts.publish(path, broadcast);
	}

	pub async fn recv_announce(&mut self, stream: &mut Stream) -> Result<(), Error> {
		let interest = stream.reader.decode::<message::AnnounceRequest>().await?;
		let prefix = interest.prefix;

		tracing::trace!(%prefix, "announce started");

		let res = self.run_announce(stream, &prefix).await;
		match res {
			Err(Error::Cancel) => {
				tracing::trace!(%prefix, "announce cancelled");
			}
			Err(err) => {
				tracing::debug!(?err, %prefix, "announce error");
			}
			_ => {
				tracing::trace!(%prefix, "announce complete");
			}
		}

		Ok(())
	}

	async fn run_announce(&mut self, stream: &mut Stream, prefix: &str) -> Result<(), Error> {
		let mut announced = self.broadcasts.announced(prefix);

		// Flush any synchronously announced paths
		loop {
			tokio::select! {
				biased;
				res = stream.reader.finished() => return res,
				announced = announced.next() => {
					match announced {
						Some(msg) => {
							tracing::debug!(?msg, "announce");
							stream.writer.encode(&msg).await?;
						},
						None => break,
					}
				}
			}
		}

		stream.writer.finish().await
	}

	pub async fn recv_subscribe(&mut self, stream: &mut Stream) -> Result<(), Error> {
		let mut subscribe = stream.reader.decode::<message::Subscribe>().await?;

		tracing::debug!(id = %subscribe.id, broadcast = %subscribe.broadcast, track = %subscribe.track, "subscribed started");

		let res = self.run_subscribe(stream, &mut subscribe).await;

		match res {
			Err(Error::Cancel) => {
				tracing::debug!(id = %subscribe.id, broadcast = %subscribe.broadcast, track = %subscribe.track, "subscribed cancelled");
			}
			Err(err) => {
				tracing::info!(?err, id = %subscribe.id, broadcast = %subscribe.broadcast, track = %subscribe.track, "subscribed error");
			}
			_ => {
				tracing::debug!(id = %subscribe.id, broadcast = %subscribe.broadcast, track = %subscribe.track, "subscribed complete");
			}
		}

		Ok(())
	}

	async fn run_subscribe(&mut self, stream: &mut Stream, subscribe: &mut message::Subscribe) -> Result<(), Error> {
		let broadcast = subscribe.broadcast.clone();
		let track = Track {
			name: subscribe.track.clone(),
			priority: subscribe.priority,
		};

		let broadcast = self.broadcasts.consume(&broadcast).ok_or(Error::NotFound)?;
		let track = broadcast.subscribe(&track);

		// TODO wait until track.info() to get the *real* priority

		let info = message::SubscribeOk {
			priority: track.info.priority,
		};

		stream.writer.encode(&info).await?;

		tokio::select! {
			res = self.run_track(track, subscribe) => res?,
			res = stream.reader.finished() => res?,
		}

		stream.writer.finish().await
	}

	async fn run_track(&mut self, mut track: TrackConsumer, subscribe: &mut message::Subscribe) -> Result<(), Error> {
		// Kind of hacky, but we only serve up to two groups concurrently.
		// This avoids a race where we try to cancel the previous group at the same time as we FIN it.
		// We don't want to allow N concurrent groups otherwise slow consumers will eat our RAM.
		// FuturesOrdered would be used if there was a `pop` method.
		let mut old_group = None;
		let mut new_group = None;

		loop {
			tokio::select! {
				Some(group) = track.next_group().transpose() => {
					let mut group = group?;

					let mut session = self.session.clone();
					let priority = Self::stream_priority(subscribe.priority, group.info.sequence);

					let msg = message::Group {
						subscribe: subscribe.id,
						sequence: group.info.sequence,
					};

					let track = track.info.clone();

					let future = Some(Box::pin(async move {
						// TODO open streams in priority order to help with MAX_STREAMS flow control issues.

						let mut stream = tokio::select! {
							biased;
							res = Writer::open(&mut session, message::DataType::Group) => res?,
							// Add a timeout to detect when we're blocked by flow control.
							_ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
								return Err(Error::Timeout);
							}
						};

						stream.set_priority(priority);

						tracing::trace!(track = %track.name, group = %group.info.sequence, "serving group");

						let res = Self::serve_group(&mut stream, msg, &mut group).await;

						match res {
							Err(Error::Cancel) => {
								tracing::trace!(track = %track.name, group = %group.info.sequence, "serving group cancelled");
								stream.abort(&Error::Cancel);
							}
							Err(err) => {
								tracing::debug!(?err, track = %track.name, group = %group.info.sequence, "serving group error");
								stream.abort(&err);
							}
							_ => {
								tracing::trace!(track = %track.name, group = %group.info.sequence, "serving group complete");
							}
						}

						Ok::<(), Error>(())
					}));

					if new_group.is_none() {
						new_group = future;
					} else {
						old_group = new_group;
						new_group = future;
					}
				},
				Some(_) = async { Some(old_group.as_mut()?.await) } => {
					old_group = None;
				},
				Some(_) = async { Some(new_group.as_mut()?.await) } => {
					new_group = None;
					old_group = None; // Also cancel the old group because it's so far behind.
				},
				// No more groups to serve.
				else => break,
			}
		}

		Ok(())
	}

	pub async fn serve_group(stream: &mut Writer, msg: message::Group, group: &mut GroupConsumer) -> Result<(), Error> {
		stream.encode(&msg).await?;

		loop {
			tokio::select! {
				biased;
				_ = stream.closed() => return Err(Error::Cancel),
				frame = group.next_frame() => {
					let mut frame = match frame? {
						Some(frame) => frame,
						None => break,
					};

					let header = message::Frame { size: frame.info.size };
					stream.encode(&header).await?;

					// Technically we should tokio::select here but who cares.
					while let Some(chunk) = frame.read().await? {
						stream.write(&chunk).await?;
					}
				}
			}
		}

		stream.finish().await
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
