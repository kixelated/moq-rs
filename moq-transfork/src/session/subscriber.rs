use std::{
	collections::{hash_map, HashMap},
	sync::{atomic, Arc},
};

use tracing::Instrument;

use crate::{
	message,
	model::{Broadcast, BroadcastConsumer, Produce, Router, Track, TrackConsumer},
	util::{spawn, Close, Lock, OrClose},
	BroadcastProducer, Error, RouterProducer,
};

use super::{subscribe, Announced, AnnouncedProducer, Reader, Session, Stream, SubscribeConsumer};

#[derive(Clone)]
pub struct Subscriber {
	session: Session,
	announced: AnnouncedProducer,

	broadcasts: Lock<HashMap<String, BroadcastConsumer>>,
	subscribes: Lock<HashMap<u64, SubscribeConsumer>>,
	next_id: Arc<atomic::AtomicU64>,
}

impl Subscriber {
	pub(super) fn new(session: Session) -> Self {
		Self {
			session,
			announced: Default::default(),

			broadcasts: Default::default(),
			subscribes: Default::default(),
			next_id: Default::default(),
		}
	}

	pub fn announced(&mut self) -> Announced {
		self.announced.subscribe()
	}

	// TODO come up with a better name
	/// Subscribe to tracks from a given broadcast.
	///
	/// This is a helper method to avoid waiting for an (optional) [Self::announced] or cloning the [Broadcast] for each [Self::subscribe].
	pub fn namespace<T: Into<Broadcast>>(&self, broadcast: T) -> Result<BroadcastConsumer, Error> {
		let broadcast = broadcast.into();
		let (mut writer, reader) = broadcast.clone().produce();

		match self.broadcasts.lock().entry(broadcast.name.clone()) {
			hash_map::Entry::Occupied(entry) => return Ok(entry.get().clone()),
			hash_map::Entry::Vacant(entry) => entry.insert(reader.clone()),
		};

		let router = Router::produce();
		writer.route_tracks(router.1);

		let announce = Announce {
			broadcast: writer,
			router: router.0,
			broadcasts: self.broadcasts.clone(),
		};

		let span = tracing::info_span!("announce", broadcast = broadcast.name);
		spawn(self.clone().run_announce(announce).instrument(span));

		Ok(reader)
	}

	async fn run_announce(self, mut announce: Announce) {
		while let Some(request) = announce.router.requested().await {
			let mut this = self.clone();
			let broadcast = announce.broadcast.info.as_ref().clone();

			spawn(async move {
				match this.subscribe(broadcast, request.info.clone()).await {
					Ok(track) => request.serve(track),
					Err(err) => request.close(err),
				};
			});
		}
	}

	pub async fn subscribe<B: Into<Broadcast>, T: Into<Track>>(
		&mut self,
		broadcast: B,
		track: T,
	) -> Result<TrackConsumer, Error> {
		self.subscribe_inner(broadcast.into(), track.into()).await
	}

	#[tracing::instrument("subscribe", skip_all, err, fields(broadcast=broadcast.name, track=track.name))]
	pub async fn subscribe_inner(&mut self, broadcast: Broadcast, track: Track) -> Result<TrackConsumer, Error> {
		let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);

		let (mut producer, consumer) = subscribe(id, track, self.subscribes.clone());
		let mut stream = self.session.open(message::Stream::Subscribe).await?;

		producer.start(&mut stream, &broadcast).await.or_close(&mut stream)?; // wait for an OK before returning
		spawn(async move {
			producer.run(&mut stream).await.or_close(&mut stream).ok();
		});

		Ok(consumer.track)
	}

	pub(super) async fn recv_announce(&mut self, stream: &mut Stream) -> Result<(), Error> {
		let announce = stream.reader.decode().await?;
		self.announced_run(stream, announce).await
	}

	#[tracing::instrument("announced", skip_all, err, fields(broadcast = announce.broadcast))]
	async fn announced_run(&mut self, stream: &mut Stream, announce: message::Announce) -> Result<(), Error> {
		// Serve the broadcast and add it to the announced queue.
		let broadcast = self.namespace(announce.broadcast)?;
		self.announced.insert(broadcast.clone());

		// Send the OK message.
		let msg = message::AnnounceOk {};
		stream.writer.encode(&msg).await?;

		tracing::info!("ok");

		// Wait until the stream is closed.
		let res = tokio::select! {
			res = stream.reader.closed() => res,
			res = broadcast.closed() => res.map_err(Into::into),
		};

		self.announced.remove(&broadcast.name);

		res
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
	broadcasts: Lock<HashMap<String, BroadcastConsumer>>,
}

impl Drop for Announce {
	fn drop(&mut self) {
		self.broadcasts.lock().remove(&self.broadcast.name);
	}
}
