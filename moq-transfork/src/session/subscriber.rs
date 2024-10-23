use std::{
	collections::HashMap,
	sync::{atomic, Arc},
};

use crate::{
	message,
	model::{Broadcast, BroadcastConsumer, Produce, Router, Track, TrackConsumer},
	util::{spawn, Close, Lock, OrClose},
	AnnouncedProducer, BroadcastProducer, Error, Path, RouterProducer,
};

use super::{subscribe, AnnouncedConsumer, Reader, Stream, SubscribeConsumer};

#[derive(Clone)]
pub(super) struct Subscriber {
	session: web_transport::Session,

	broadcasts: AnnouncedProducer,
	subscribes: Lock<HashMap<u64, SubscribeConsumer>>,
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

	/// Discover any broadcasts matching a prefix.
	pub fn broadcasts(&self, prefix: Path) -> AnnouncedConsumer {
		let producer = AnnouncedProducer::default();
		let consumer = producer.subscribe_prefix(prefix.clone());

		let mut this = self.clone();
		spawn(async move {
			let mut stream = match Stream::open(&mut this.session, message::Stream::Announce).await {
				Ok(stream) => stream,
				Err(err) => {
					tracing::warn!(?err, "failed to open announce stream");
					return;
				}
			};

			this.run_announce(&mut stream, prefix, producer)
				.await
				.or_close(&mut stream)
				.ok();
		});

		consumer
	}

	async fn run_announce(&self, stream: &mut Stream, prefix: Path, announced: AnnouncedProducer) -> Result<(), Error> {
		// Used to remove broadcasts on ended
		let mut active = HashMap::new();

		stream
			.writer
			.encode(&message::AnnounceInterest { prefix: prefix.clone() })
			.await?;

		tracing::debug!("waiting for announces");

		loop {
			tokio::select! {
				res = stream.reader.decode_maybe::<message::Announce>() => {
					match res? {
						// Handle the announce
						Some(announce) => {
							tracing::debug!(?announce);

							let suffix = announce.suffix;

							match announce.status {
								message::AnnounceStatus::Active => {
									if active.contains_key(&suffix) {
										return Err(Error::Duplicate);
									}

									let path = prefix.clone().append(&suffix);
									let broadcast = self.broadcast(Broadcast::new(path));
									let broadcast = announced.insert(broadcast)?;

									active.insert(suffix, broadcast);
								},
								message::AnnounceStatus::Ended => {
									if active.remove(&suffix).is_none() {
										return Err(Error::NotFound);
									}
								},
							}
						},
						// Stop if the stream has been closed
						None => return Ok(()),
					}
				},
				// Stop if the consumer is no longer interested
				_ = announced.closed() => return Ok(()),
			}
		}
	}

	/// Subscribe to tracks from a given broadcast.
	///
	/// This is a helper method to avoid waiting for an (optional) [Self::announced] or cloning the [Broadcast] for each [Self::subscribe].
	#[tracing::instrument("subscribe", skip_all, fields(?broadcast))]
	pub fn broadcast(&self, broadcast: Broadcast) -> BroadcastConsumer {
		if let Some(broadcast) = self.broadcasts.get(&broadcast) {
			return broadcast;
		}

		let (mut writer, reader) = broadcast.clone().produce();
		let served = self
			.broadcasts
			.insert(reader.clone())
			.expect("race I'm too lazy to handle");

		let router = Router::produce();
		writer.route_tracks(router.1);

		let this = self.clone();
		let session = self.session.clone();

		spawn(async move {
			tokio::select! {
				_ = this.run_router(writer, router.0) => (),
				_ = session.closed() => (),
			};

			drop(served);
		});

		reader
	}

	async fn run_router(self, broadcast: BroadcastProducer, mut router: RouterProducer<Track>) {
		while let Some(request) = router.requested().await {
			let mut this = self.clone();
			let broadcast = broadcast.info.clone();
			let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);

			spawn(async move {
				match this.subscribe_track(id, broadcast, request.info.clone()).await {
					Ok(track) => request.serve(track),
					Err(err) => request.close(err),
				};
			});
		}
	}

	#[tracing::instrument("subscribe", skip_all, err, fields(id, ?broadcast, ?track))]
	pub async fn subscribe_track(
		&mut self,
		id: u64,
		broadcast: Broadcast,
		track: Track,
	) -> Result<TrackConsumer, Error> {
		let (mut producer, consumer) = subscribe(id, track, self.subscribes.clone());
		let mut stream = Stream::open(&mut self.session, message::Stream::Subscribe).await?;

		if let Err(err) = producer.start(&mut stream, &broadcast).await {
			tracing::warn!(?err, "failed");

			stream.close(err.clone());
			return Err(err);
		}

		spawn(async move {
			producer.run(&mut stream).await.or_close(&mut stream).ok();
		});

		Ok(consumer.track)
	}

	pub async fn recv_group(&mut self, mut stream: Reader) {
		match self.find_subscribe(&mut stream).await {
			Ok((group, subscribe)) => subscribe.serve(group, stream).await,
			Err(err) => stream.close(err),
		}
	}

	async fn find_subscribe(&mut self, stream: &mut Reader) -> Result<(message::Group, SubscribeConsumer), Error> {
		let group: message::Group = stream.decode().await?;

		let subscribe = self
			.subscribes
			.lock()
			.get(&group.subscribe)
			.cloned()
			.ok_or(Error::NotFound)?;

		Ok((group, subscribe))
	}
}
