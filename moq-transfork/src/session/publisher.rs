use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	message,
	model::{Broadcast, BroadcastConsumer, GroupConsumer, Track, TrackConsumer},
	util::{spawn, FuturesExt, OrClose},
	AnnouncedProducer, Error,
};

use super::{Stream, Writer};

#[derive(Clone)]
pub(super) struct Publisher {
	session: web_transport::Session,
	announced: AnnouncedProducer,
}

impl Publisher {
	pub fn new(session: web_transport::Session) -> Self {
		Self {
			session,
			announced: Default::default(),
		}
	}

	/// Announce a broadcast.
	#[tracing::instrument("publish", skip_all, err, fields(?broadcast))]
	pub fn publish(&mut self, broadcast: BroadcastConsumer) -> Result<(), Error> {
		let active = self.announced.insert(broadcast.clone())?;
		let session = self.session.clone();

		spawn(async move {
			tokio::select! {
				_ = broadcast.closed() => (),
				_ = session.closed() => (),
			}
			drop(active);
		});

		Ok(())
	}

	pub async fn recv_announce(&mut self, stream: &mut Stream) -> Result<(), Error> {
		let interest = stream.reader.decode::<message::AnnounceInterest>().await?;
		let prefix = interest.prefix;
		tracing::debug!(?prefix, "announced interest");

		let mut unannounced = FuturesUnordered::new();
		let mut announced = self.announced.subscribe_prefix(prefix.clone());

		loop {
			tokio::select! {
				Some(broadcast) = announced.next() => {
					tracing::debug!(announced = ?broadcast.info);

					let suffix = broadcast.info.path.clone().strip_prefix(&prefix).expect("prefix mismatch");

					stream.writer.encode(&message::Announce {
						status: message::AnnounceStatus::Active,
						suffix,
					}).await?;

					unannounced.push(async move {
						broadcast.closed().await.ok();
						broadcast
					});
				},
				Some(broadcast) = unannounced.next() => {
					tracing::debug!(unannounced = ?broadcast.info);

					let suffix = broadcast.info.path.strip_prefix(&prefix).expect("prefix mismatch");

					stream.writer.encode(&message::Announce {
						status: message::AnnounceStatus::Ended,
						suffix,
					}).await?;
				},
				res = stream.reader.closed() => return res,
			}
		}
	}

	async fn subscribe<B: Into<Broadcast>, T: Into<Track>>(
		&self,
		broadcast: B,
		track: T,
	) -> Result<TrackConsumer, Error> {
		let broadcast = broadcast.into();
		let track = track.into();

		let reader = self.announced.get(&broadcast);
		if let Some(broadcast) = reader {
			return broadcast.get_track(track).await;
		}

		Err(Error::NotFound)
	}

	pub async fn recv_subscribe(&mut self, stream: &mut Stream) -> Result<(), Error> {
		let subscribe = stream.reader.decode().await?;
		self.serve_subscribe(stream, subscribe).await
	}

	#[tracing::instrument("publish", skip_all, err, fields(broadcast = ?subscribe.broadcast, track = subscribe.track, id = subscribe.id))]
	async fn serve_subscribe(&mut self, stream: &mut Stream, subscribe: message::Subscribe) -> Result<(), Error> {
		let track = Track {
			name: subscribe.track,
			priority: subscribe.priority,
			group_expires: subscribe.group_expires,
			group_order: subscribe.group_order,
		};

		let broadcast = Broadcast::new(subscribe.broadcast);
		let mut track = self.subscribe(broadcast, track).await?;

		let info = message::Info {
			group_latest: track.latest_group(),
			group_expires: track.group_expires,
			group_order: track.group_order,
			track_priority: track.priority,
		};

		stream.writer.encode(&info).await?;

		tracing::info!("active");

		let mut tasks = FuturesUnordered::new();
		let mut complete = false;

		loop {
			tokio::select! {
				Some(group) = track.next_group().transpose() => {
					let mut group = group?;
					let session = self.session.clone();

					tasks.push(async move {
						let res = Self::serve_group(session, subscribe.id, &mut group).await;
						(group, res)
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
				Some(res) = tasks.next() => {
					let (group, res) = res;

					if let Err(err) = res {
						let drop = message::GroupDrop {
							sequence: group.sequence,
							count: 0,
							code: err.to_code(),
						};

						stream.writer.encode(&drop).await?;
					}
				},
				else => break,
			}
		}

		tracing::info!("done");

		Ok(())
	}

	#[tracing::instrument("data", skip_all, err, fields(?subscribe, group = group.sequence))]
	pub async fn serve_group(
		mut session: web_transport::Session,
		subscribe: u64,
		group: &mut GroupConsumer,
	) -> Result<(), Error> {
		let mut stream = Writer::open(&mut session, message::StreamUni::Group).await?;

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
			sequence: group.sequence,
		};

		stream.encode(&msg).await?;

		let mut frames = 0;

		while let Some(mut frame) = group.next_frame().await? {
			let header = message::Frame { size: frame.size };
			stream.encode(&header).await?;

			let mut remain = frame.size;

			while let Some(chunk) = frame.read().await? {
				remain = remain.checked_sub(chunk.len()).ok_or(Error::WrongSize)?;
				tracing::trace!(chunk = chunk.len(), remain, "chunk");

				stream.write(&chunk).await?;
			}

			if remain > 0 {
				return Err(Error::WrongSize);
			}

			frames += 1;
		}

		tracing::debug!(frames, "served");

		// TODO block until all bytes have been acknowledged so we can still reset
		// writer.finish().await?;

		Ok(())
	}

	pub async fn recv_fetch(&mut self, stream: &mut Stream) -> Result<(), Error> {
		let fetch = stream.reader.decode().await?;
		self.serve_fetch(stream, fetch).await
	}

	#[tracing::instrument("fetch", skip_all, err, fields(broadcast = ?fetch.broadcast, track = fetch.track, group = fetch.group, offset = fetch.offset))]
	async fn serve_fetch(&mut self, _stream: &mut Stream, fetch: message::Fetch) -> Result<(), Error> {
		let track = Track::build(fetch.track).priority(fetch.priority);
		let track = self.subscribe(fetch.broadcast, track).await?;
		let _group = track.get_group(fetch.group)?;

		unimplemented!("TODO fetch");
	}

	pub async fn recv_info(&mut self, stream: &mut Stream) -> Result<(), Error> {
		let info = stream.reader.decode().await?;
		self.serve_info(stream, info).await
	}

	#[tracing::instrument("info", skip_all, err, fields(broadcast = ?info.broadcast, track = info.track))]
	async fn serve_info(&mut self, stream: &mut Stream, info: message::InfoRequest) -> Result<(), Error> {
		let track = self.subscribe(info.broadcast, info.track).await?;

		let info = message::Info {
			group_latest: track.latest_group(),
			track_priority: track.priority,
			group_expires: track.group_expires,
			group_order: track.group_order,
		};

		stream.writer.encode(&info).await?;

		Ok(())
	}
}
