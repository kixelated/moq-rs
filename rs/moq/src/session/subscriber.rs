use std::{
	collections::HashMap,
	sync::{atomic, Arc},
};

use crate::{
	message,
	model::{BroadcastConsumer, BroadcastProducer},
	AnnouncedProducer, Broadcast, Error, Frame, FrameProducer, Group, GroupProducer, TrackProducer,
};

use web_async::{spawn, Lock};

use super::{AnnouncedConsumer, Reader, Stream};

#[derive(Clone)]
pub(super) struct Subscriber {
	session: web_transport::Session,

	broadcasts: Lock<HashMap<String, BroadcastProducer>>,
	subscribes: Lock<HashMap<u64, TrackProducer>>,
	next_id: Arc<atomic::AtomicU64>,
}

impl Subscriber {
	pub fn new(session: web_transport::Session) -> Self {
		Self {
			session,

			broadcasts: Default::default(),
			subscribes: Default::default(),
			next_id: Default::default(),
		}
	}

	/// Discover any tracks matching a filter.
	pub fn announced<S: ToString>(&self, prefix: S) -> AnnouncedConsumer {
		let prefix = prefix.to_string();

		let producer = AnnouncedProducer::default();
		let consumer = producer.consume(prefix.clone());

		web_async::spawn(self.clone().run_announced(prefix, producer));

		consumer
	}

	async fn run_announced(mut self, prefix: String, producer: AnnouncedProducer) {
		tracing::debug!(%prefix, "announced started");

		// Keep running until we don't care about the producer anymore.
		let closed = producer.clone();

		// Wait until the producer is no longer needed or the stream is closed.
		let res = tokio::select! {
			_ = closed.unused() => Err(Error::Cancel),
			res = self.run_announce(&prefix, producer) => res,
		};

		match res {
			Err(Error::Cancel) => {
				tracing::debug!(%prefix, "announced cancelled");
			}
			Err(err) => {
				tracing::warn!(?err, %prefix, "announced error");
			}
			_ => {
				tracing::debug!(%prefix, "announced complete");
			}
		}
	}

	async fn run_announce(&mut self, prefix: &str, mut announced: AnnouncedProducer) -> Result<(), Error> {
		let mut stream = Stream::open(&mut self.session, message::ControlType::Announce).await?;

		stream
			.writer
			.encode(&message::AnnounceRequest {
				prefix: prefix.to_string(),
			})
			.await?;

		while let Some(announce) = stream.reader.decode_maybe::<message::Announce>().await? {
			match announce {
				message::Announce::Active { suffix } => {
					let broadcast = match prefix {
						"" => Broadcast { path: suffix },
						prefix => Broadcast {
							path: format!("{}{}", prefix, suffix),
						},
					};

					tracing::debug!(broadcast = %broadcast.path, "received announce");

					if !announced.insert(broadcast) {
						return Err(Error::Duplicate);
					}
				}
				message::Announce::Ended { suffix } => {
					let broadcast = match prefix {
						"" => Broadcast { path: suffix },
						prefix => Broadcast {
							path: format!("{}{}", prefix, suffix),
						},
					};

					tracing::debug!(broadcast = %broadcast.path, "received unannounce");

					if !announced.remove(&broadcast) {
						return Err(Error::NotFound);
					}
				}
			}
		}

		// Close the writer.
		stream.writer.finish().await
	}

	/// Subscribe to a given broadcast.
	pub fn consume(&self, broadcast: &Broadcast) -> BroadcastConsumer {
		if let Some(producer) = self.broadcasts.lock().get(&broadcast.path) {
			return producer.consume();
		}

		let producer = broadcast.clone().produce();
		let consumer = producer.consume();

		// Run the broadcast in the background until all consumers are dropped.
		spawn(self.clone().run_broadcast(producer));

		consumer
	}

	async fn run_broadcast(self, broadcast: BroadcastProducer) {
		// Actually start serving subscriptions.
		loop {
			// Keep serving requests until there are no more consumers.
			// This way we'll clean up the task when the broadcast is no longer needed.
			let producer = tokio::select! {
				producer = broadcast.requested() => producer,
				_ = broadcast.unused() => break,
				_ = self.session.closed() => break,
			};

			let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);
			spawn(self.clone().run_subscribe(id, broadcast.clone(), producer));
		}

		// Remove the broadcast from the lookup.
		self.broadcasts.lock().remove(&broadcast.info.path);
	}

	async fn run_subscribe(mut self, id: u64, broadcast: BroadcastProducer, mut track: TrackProducer) {
		self.subscribes.lock().insert(id, track.clone());

		let msg = message::Subscribe {
			id,
			broadcast: broadcast.info.path.clone(),
			track: track.info.name.clone(),
			priority: track.info.priority,
		};

		tracing::info!(broadcast = %broadcast.info.path, track = %track.info.name, id, "subscription started");

		let res = tokio::select! {
			_ = track.unused() => Err(Error::Cancel),
			res = self.run_track(msg) => res,
		};

		match res {
			Err(Error::Cancel) => {
				tracing::info!(broadcast = %broadcast.info.path, track = %track.info.name, id, "subscription cancelled");
				track.abort(Error::Cancel);
			}
			Err(err) => {
				tracing::warn!(?err, broadcast = %broadcast.info.path, track = %track.info.name, id, "subscription error");
				track.abort(err);
			}
			_ => {
				tracing::info!(broadcast = %broadcast.info.path, track = %track.info.name, id, "subscription complete");
				track.finish();
			}
		}

		self.subscribes.lock().remove(&id);
		broadcast.remove(&track.info.name);
	}

	async fn run_track(&mut self, msg: message::Subscribe) -> Result<(), Error> {
		let mut stream = Stream::open(&mut self.session, message::ControlType::Subscribe).await?;

		if let Err(err) = self.run_track_stream(&mut stream, msg).await {
			stream.writer.abort(&err);
			return Err(err);
		}

		stream.writer.finish().await
	}

	async fn run_track_stream(&mut self, stream: &mut Stream, msg: message::Subscribe) -> Result<(), Error> {
		stream.writer.encode(&msg).await?;

		// TODO use the response correctly populate the track info
		let _info: message::SubscribeOk = stream.reader.decode().await?;

		// Wait until the stream is closed
		stream.reader.finished().await?;

		Ok(())
	}

	pub async fn recv_group(&mut self, stream: &mut Reader) -> Result<(), Error> {
		let group: message::Group = stream.decode().await?;

		tracing::trace!(group = %group.sequence, "received group");

		let mut group = {
			let mut subs = self.subscribes.lock();
			let track = subs.get_mut(&group.subscribe).ok_or(Error::Cancel)?;

			let group = Group {
				sequence: group.sequence,
			};
			track.create_group(group).ok_or(Error::Old)?
		};

		let res = tokio::select! {
			_ = group.unused() => Err(Error::Cancel),
			res = self.run_group(stream, group.clone()) => res,
		};

		match res {
			Err(Error::Cancel) => {
				tracing::trace!(group = %group.info.sequence, "group cancelled");
				group.abort(Error::Cancel);
			}
			Err(err) => {
				tracing::warn!(?err, group = %group.info.sequence, "group error");
				group.abort(err);
			}
			_ => {
				tracing::trace!(group = %group.info.sequence, "group complete");
				group.finish();
			}
		}

		Ok(())
	}

	async fn run_group(&mut self, stream: &mut Reader, mut group: GroupProducer) -> Result<(), Error> {
		while let Some(frame) = stream.decode_maybe::<message::Frame>().await? {
			let mut frame = group.create_frame(Frame { size: frame.size });

			let res = tokio::select! {
				_ = frame.unused() => Err(Error::Cancel),
				res = self.run_frame(stream, frame.clone()) => res,
			};

			if let Err(err) = res {
				frame.abort(err.clone());
				return Err(err);
			}
		}

		group.finish();

		Ok(())
	}

	async fn run_frame(&mut self, stream: &mut Reader, mut frame: FrameProducer) -> Result<(), Error> {
		let mut remain = frame.info.size;

		while remain > 0 {
			let chunk = stream.read(remain as usize).await?.ok_or(Error::WrongSize)?;
			remain = remain.checked_sub(chunk.len() as u64).ok_or(Error::WrongSize)?;
			frame.write(chunk);
		}

		frame.finish();

		Ok(())
	}
}
