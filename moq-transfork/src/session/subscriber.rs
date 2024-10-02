use std::{
	collections::{hash_map, HashMap},
	sync::{atomic, Arc},
};

use crate::{
	message,
	model::{Broadcast, BroadcastConsumer, Produce, Router, Track, TrackConsumer},
	util::{spawn, Close, Lock, OrClose},
	BroadcastProducer, Error, RouterProducer,
};

use super::{subscribe, AnnouncedConsumer, AnnouncedProducer, Reader, Session, Stream, SubscribeConsumer};

#[derive(Clone)]
pub struct Subscriber {
	session: Session,

	broadcasts: Lock<HashMap<Broadcast, BroadcastConsumer>>,
	subscribes: Lock<HashMap<u64, SubscribeConsumer>>,
	next_id: Arc<atomic::AtomicU64>,
}

impl Subscriber {
	pub(super) fn new(session: Session) -> Self {
		Self {
			session,

			broadcasts: Default::default(),
			subscribes: Default::default(),
			next_id: Default::default(),
		}
	}

	/// Discover any broadcasts.
	pub fn announced(&mut self) -> AnnouncedConsumer {
		self.announced_prefix("")
	}

	/// Discover any broadcasts matching a prefix.
	///
	/// This function is synchronous unless the connection is blocked on flow control.
	#[tracing::instrument("announced", skip_all, fields(prefix = %prefix.to_string()))]
	pub fn announced_prefix<P: ToString>(&mut self, prefix: P) -> AnnouncedConsumer {
		let producer = AnnouncedProducer::default();
		let prefix = prefix.to_string();
		let consumer = producer.subscribe_prefix(prefix.clone());

		let mut this = self.clone();
		spawn(async move {
			let mut stream = match this.session.open(message::Stream::Announce).await {
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

	async fn run_announce(
		&self,
		stream: &mut Stream,
		prefix: String,
		mut announced: AnnouncedProducer,
	) -> Result<(), Error> {
		// Used to toggle on each duplicate announce
		let mut active = HashMap::new();

		stream.writer.encode(&message::AnnounceInterest { prefix }).await?;

		tracing::debug!("waiting for announces");

		loop {
			tokio::select! {
				res = stream.reader.decode_maybe::<message::Announce>() => {
					match res? {
						Some(announce) => {
							tracing::debug!(?announce);
							let broadcast = self.subscribe(announce.broadcast);

							match active.entry(broadcast.info.name.clone()) {
								hash_map::Entry::Occupied(entry) => &mut entry.remove(),
								hash_map::Entry::Vacant(entry) => entry.insert(announced.insert(broadcast)?),
							};
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
	pub fn subscribe<T: Into<Broadcast>>(&self, broadcast: T) -> BroadcastConsumer {
		let broadcast = broadcast.into();
		let (mut writer, reader) = broadcast.clone().produce();

		match self.broadcasts.lock().entry(broadcast.clone()) {
			hash_map::Entry::Occupied(entry) => return entry.get().clone(),
			hash_map::Entry::Vacant(entry) => entry.insert(reader.clone()),
		};

		let router = Router::produce();
		writer.route_tracks(router.1);

		let announce = Announce {
			broadcast: writer,
			router: router.0,
			broadcasts: self.broadcasts.clone(),
		};

		spawn(self.clone().run_router(announce));

		reader
	}

	#[tracing::instrument("announced", skip_all, fields(broadcast = %announce.broadcast.info.name))]
	async fn run_router(self, mut announce: Announce) {
		while let Some(request) = announce.router.requested().await {
			println!("request: {:?}", request.info);
			let mut this = self.clone();
			let broadcast = announce.broadcast.info.clone();

			spawn(async move {
				match this.subscribe_track(broadcast, request.info.clone()).await {
					Ok(track) => request.serve(track),
					Err(err) => request.close(err),
				};
			});
		}
	}

	async fn subscribe_track<B: Into<Broadcast>, T: Into<Track>>(
		&mut self,
		broadcast: B,
		track: T,
	) -> Result<TrackConsumer, Error> {
		let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);
		self.subscribe_inner(id, broadcast.into(), track.into()).await
	}

	#[tracing::instrument("subscribe", skip_all, err, fields(id, broadcast=%broadcast.name, track=track.name))]
	pub async fn subscribe_inner(
		&mut self,
		id: u64,
		broadcast: Broadcast,
		track: Track,
	) -> Result<TrackConsumer, Error> {
		let (mut producer, consumer) = subscribe(id, track, self.subscribes.clone());
		let mut stream = self.session.open(message::Stream::Subscribe).await?;

		if let Err(err) = producer.start(&mut stream, &broadcast).await {
			tracing::warn!(?err, "failed");

			stream.close(err.clone());
			return Err(err);
		}

		tracing::info!("active");

		spawn(async move {
			producer.run(&mut stream).await.or_close(&mut stream).ok();
		});

		Ok(consumer.track)
	}

	pub(super) async fn recv_group(&mut self, mut stream: Reader) {
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

	pub async fn closed(&self) -> Result<(), Error> {
		self.session.closed().await
	}
}

struct Announce {
	pub broadcast: BroadcastProducer,
	pub router: RouterProducer<Track>,
	broadcasts: Lock<HashMap<Broadcast, BroadcastConsumer>>,
}

impl Drop for Announce {
	fn drop(&mut self) {
		self.broadcasts.lock().remove(&self.broadcast.info);
	}
}
