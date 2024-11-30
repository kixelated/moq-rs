use std::{
	collections::{hash_map, HashMap},
	sync::{atomic, Arc},
};

use crate::{
	message,
	model::{Track, TrackConsumer},
	util::{spawn, Lock, OrClose},
	AnnouncedProducer, Error, Path, TrackProducer,
};

use super::{AnnouncedConsumer, Reader, Stream};

#[derive(Clone)]
pub(super) struct Subscriber {
	session: web_transport::Session,

	tracks: Lock<HashMap<Path, TrackProducer>>,
	subscribes: Lock<HashMap<u64, TrackProducer>>,
	next_id: Arc<atomic::AtomicU64>,
}

impl Subscriber {
	pub fn new(session: web_transport::Session) -> Self {
		Self {
			session,

			tracks: Default::default(),
			subscribes: Default::default(),
			next_id: Default::default(),
		}
	}

	/// Discover any tracks matching a prefix.
	pub fn announced(&self, prefix: Path) -> AnnouncedConsumer {
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

	async fn run_announce(
		&self,
		stream: &mut Stream,
		prefix: Path,
		mut announced: AnnouncedProducer,
	) -> Result<(), Error> {
		stream
			.writer
			.encode(&message::AnnounceInterest { prefix: prefix.clone() })
			.await?;

		tracing::debug!(?prefix, "waiting for announcements");

		loop {
			tokio::select! {
				res = stream.reader.decode_maybe::<message::Announce>() => {
					match res? {
						// Handle the announce
						Some(announce) => {
							let path = prefix.clone().append(&announce.suffix);
							tracing::debug!(?path, ?announce.status, "announce");

							match announce.status {
								message::AnnounceStatus::Active => {
									tracing::debug!(?path, "announced");
									if !announced.announce(path) {
										return Err(Error::Duplicate);
									}
								},
								message::AnnounceStatus::Ended => {
									tracing::debug!(?path, "unannounced");
									if !announced.unannounce(&path) {
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

	/// Subscribe to a given track.
	pub fn subscribe(&self, track: Track) -> TrackConsumer {
		let path = track.path.clone();
		let (writer, reader) = track.clone().produce();

		// Check if we can deduplicate this subscription
		match self.tracks.lock().entry(path.clone()) {
			hash_map::Entry::Occupied(entry) => return entry.get().subscribe(),
			hash_map::Entry::Vacant(entry) => entry.insert(writer.clone()),
		};

		let mut this = self.clone();
		let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);

		spawn(async move {
			if let Ok(mut stream) = Stream::open(&mut this.session, message::Stream::Subscribe).await {
				this.run_subscribe(id, writer, &mut stream)
					.await
					.or_close(&mut stream)
					.ok();
			}

			this.subscribes.lock().remove(&id);
			this.tracks.lock().remove(&path);
		});

		reader
	}

	#[tracing::instrument("subscribe", skip_all, fields(?id, track = ?track.path))]
	async fn run_subscribe(&mut self, id: u64, track: TrackProducer, stream: &mut Stream) -> Result<(), Error> {
		let request = message::Subscribe {
			id,
			path: track.path.clone(),
			priority: track.priority,

			group_order: track.group_order,
			group_expires: track.group_expires,

			// TODO
			group_min: None,
			group_max: None,
		};

		stream.writer.encode(&request).await?;

		// TODO use the response to correctly populate the track info
		let _response: message::Info = stream.reader.decode().await?;

		tracing::info!("active");

		self.subscribes.lock().insert(id, track.clone());

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

		tracing::info!("done");

		Ok(())
	}

	pub async fn recv_group(&mut self, stream: &mut Reader) -> Result<(), Error> {
		let group: message::Group = stream.decode().await?;
		tracing::trace!(?group, "decoded group");

		let mut group = {
			let mut subs = self.subscribes.lock();
			let track = subs.get_mut(&group.subscribe).ok_or(Error::NotFound)?;
			track.create_group(group.sequence)
		};

		while let Some(frame) = stream.decode_maybe::<message::Frame>().await? {
			tracing::trace!(?frame, "decoded frame");
			let mut frame = group.create_frame(frame.size);
			let mut remain = frame.size;

			while remain > 0 {
				let chunk = stream.read(remain).await?.ok_or(Error::WrongSize)?;

				remain = remain.checked_sub(chunk.len()).ok_or(Error::WrongSize)?;
				tracing::trace!(chunk = chunk.len(), remain, "chunk");

				frame.write(chunk);
			}
		}

		Ok(())
	}
}
