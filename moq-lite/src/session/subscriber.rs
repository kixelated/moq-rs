use std::{
	collections::HashMap,
	sync::{atomic, Arc},
};

use crate::{
	message,
	model::{BroadcastConsumer, BroadcastProducer},
	AnnouncedProducer, Broadcast, Error, Frame, Group, TrackProducer,
};

use web_async::{spawn, Lock};

use super::{AnnouncedConsumer, OrClose, Reader, Stream};

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

		let mut session = self.session.clone();
		web_async::spawn(async move {
			let mut stream = match Stream::open(&mut session, message::ControlType::Announce).await {
				Ok(stream) => stream,
				Err(err) => {
					tracing::warn!(?err, "failed to open announce stream");
					return;
				}
			};

			if let Err(err) = Self::run_announce(&mut stream, prefix, producer)
				.await
				.or_close(&mut stream)
			{
				tracing::warn!(?err, "announced error");
			}
		});

		consumer
	}

	async fn run_announce(stream: &mut Stream, prefix: String, mut announced: AnnouncedProducer) -> Result<(), Error> {
		tracing::debug!(%prefix, "receiving announcements");

		stream
			.writer
			.encode(&message::AnnounceRequest { prefix: prefix.clone() })
			.await?;

		loop {
			tokio::select! {
				res = stream.reader.decode_maybe::<message::Announce>() => {
					match res? {
						// Handle the announce
						Some(announce) => Self::recv_announce(announce, &prefix, &mut announced)?,
						// Stop if the stream has been closed
						None => return Ok(()),
					}
				},
				// Stop if the consumer is no longer interested
				_ = announced.closed() => return Ok(()),
			}
		}
	}

	fn recv_announce(
		announce: message::Announce,
		prefix: &str,
		announced: &mut AnnouncedProducer,
	) -> Result<(), Error> {
		match announce {
			message::Announce::Active { suffix } => {
				let broadcast = match prefix {
					"" => Broadcast::new(suffix),
					prefix => Broadcast::new(format!("{}{}", prefix, suffix)),
				};

				tracing::debug!(broadcast = %broadcast.path, "received announced");

				if !announced.insert(broadcast) {
					return Err(Error::Duplicate);
				}
			}
			message::Announce::Ended { suffix } => {
				let broadcast = match prefix {
					"" => Broadcast::new(suffix),
					prefix => Broadcast::new(format!("{}{}", prefix, suffix)),
				};

				tracing::debug!(broadcast = %broadcast.path, "received unannounced");

				if !announced.remove(&broadcast) {
					return Err(Error::NotFound);
				}
			}
		}
		Ok(())
	}

	/// Subscribe to a given broadcast.
	pub fn consume(&self, broadcast: Broadcast) -> BroadcastConsumer {
		if let Some(producer) = self.broadcasts.lock().get(&broadcast.path) {
			return producer.consume();
		}

		let producer = broadcast.produce();
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

			// Insert the request into the lookup.
			// TODO This seems useful as part of BroadcastProducer in general?
			broadcast.insert(producer.consume());

			let mut this = self.clone();
			let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);
			let broadcast = broadcast.clone();

			spawn(async move {
				let track = producer.info.clone();

				if let Ok(mut stream) = Stream::open(&mut this.session, message::ControlType::Subscribe).await {
					if let Err(err) = this
						.run_subscribe(id, &broadcast.info, producer, &mut stream)
						.await
						.or_close(&mut stream)
					{
						tracing::warn!(?err, "subscribe error");
					}
				}

				this.subscribes.lock().remove(&id);
				broadcast.remove(&track.name);
			});
		}

		// Remove the broadcast from the lookup.
		self.broadcasts.lock().remove(&broadcast.info.path);
	}

	async fn run_subscribe(
		&mut self,
		id: u64,
		broadcast: &Broadcast,
		track: TrackProducer,
		stream: &mut Stream,
	) -> Result<(), Error> {
		self.subscribes.lock().insert(id, track.clone());

		let request = message::Subscribe {
			id,
			broadcast: broadcast.path.clone(),
			track: track.info.name.clone(),
			priority: track.info.priority,
		};

		stream.writer.encode(&request).await?;

		// TODO use the response correctly populate the track info
		let _info: message::SubscribeOk = stream.reader.decode().await?;

		loop {
			tokio::select! {
				res = stream.reader.decode_maybe::<message::GroupDrop>() => {
					match res? {
						Some(drop) => {
							tracing::info!(?drop, "dropped");
							// TODO expose updates to application
							// TODO use to detect gaps
						},
						None => break,
					}
				}
				// Close when there are no more subscribers
				_ = track.unused() => break
			};
		}

		tracing::info!(broadcast = %broadcast.path, track = %track.info.name, id, "subscription complete");

		Ok(())
	}

	pub async fn recv_group(&mut self, stream: &mut Reader) -> Result<(), Error> {
		let group = stream.decode().await?;
		self.recv_group_inner(stream, group).await.or_close(stream)
	}

	pub async fn recv_group_inner(&mut self, stream: &mut Reader, group: message::Group) -> Result<(), Error> {
		let mut group = {
			let mut subs = self.subscribes.lock();
			let track = subs.get_mut(&group.subscribe).ok_or(Error::Cancel)?;

			let group = Group {
				sequence: group.sequence,
			};
			track.create_group(group).ok_or(Error::Old)?
		};

		while let Some(frame) = stream.decode_maybe::<message::Frame>().await? {
			let frame = Frame { size: frame.size };
			let mut remain = frame.size;
			let mut frame = group.create_frame(frame);

			while remain > 0 {
				let chunk = stream.read(remain as usize).await?.ok_or(Error::WrongSize)?;

				remain = remain.checked_sub(chunk.len() as u64).ok_or(Error::WrongSize)?;
				frame.write(chunk);
			}
		}

		Ok(())
	}
}
