use std::collections::{hash_map, HashMap};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	message,
	model::{GroupConsumer, Track, TrackConsumer},
	util::{spawn, FuturesExt, Lock, OrClose},
	AnnouncedGuard, AnnouncedProducer, Error, Path, RouterConsumer,
};

use super::{Stream, Writer};

#[derive(Clone)]
pub(super) struct Publisher {
	session: web_transport::Session,
	announced: AnnouncedProducer,
	tracks: Lock<HashMap<Path, TrackConsumer>>,
	router: Option<RouterConsumer>,
}

impl Publisher {
	pub fn new(session: web_transport::Session) -> Self {
		Self {
			session,
			announced: Default::default(),
			tracks: Default::default(),
			router: Default::default(),
		}
	}

	/// Manually announce a path
	pub fn announce(&self, path: Path) -> Result<AnnouncedGuard, Error> {
		self.announced.insert(path)
	}

	/// Publish a track.
	#[tracing::instrument("publish", skip_all, err, fields(?track))]
	pub fn publish(&mut self, track: TrackConsumer) -> Result<(), Error> {
		match self.tracks.lock().entry(track.path.clone()) {
			hash_map::Entry::Occupied(_) => return Err(Error::Duplicate),
			hash_map::Entry::Vacant(entry) => entry.insert(track.clone()),
		};

		let active = self.announced.insert(track.path.clone())?;
		let this = self.clone();

		spawn(async move {
			tokio::select! {
				_ = track.closed() => (),
				_ = this.session.closed() => (),
			}
			this.tracks.lock().remove(&track.path);
			drop(active);
		});

		Ok(())
	}

	/// Optionally support requests for arbitrary paths using the provided router.
	/// This is useful when producing tracks dynamically.
	pub fn route(&mut self, router: Option<RouterConsumer>) {
		self.router = router;
	}

	pub async fn recv_announce(&mut self, stream: &mut Stream) -> Result<(), Error> {
		let interest = stream.reader.decode::<message::AnnounceInterest>().await?;
		let prefix = interest.prefix;
		tracing::debug!(?prefix, "announced interest");

		let mut unannounced = FuturesUnordered::new();
		let mut announced = self.announced.subscribe_prefix(prefix.clone());

		loop {
			tokio::select! {
				Some(announced) = announced.next() => {
					tracing::debug!(announced = ?announced.path);

					let suffix = announced.path.clone().strip_prefix(&prefix).expect("prefix mismatch");

					stream.writer.encode(&message::Announce {
						status: message::AnnounceStatus::Active,
						suffix,
					}).await?;

					unannounced.push(async move {
						announced.closed().await;
						announced.path
					});
				},
				Some(path) = unannounced.next() => {
					tracing::debug!(unannounced = ?path);

					let suffix = path.clone().strip_prefix(&prefix).expect("prefix mismatch");

					stream.writer.encode(&message::Announce {
						status: message::AnnounceStatus::Ended,
						suffix,
					}).await?;
				},
				res = stream.reader.closed() => return res,
			}
		}
	}

	pub async fn recv_subscribe(&mut self, stream: &mut Stream) -> Result<(), Error> {
		let subscribe = stream.reader.decode().await?;
		self.serve_subscribe(stream, subscribe).await
	}

	#[tracing::instrument("publish", skip_all, err, fields(track = ?subscribe.path, id = subscribe.id))]
	async fn serve_subscribe(&mut self, stream: &mut Stream, subscribe: message::Subscribe) -> Result<(), Error> {
		let track = Track {
			path: subscribe.path,
			priority: subscribe.priority,
			group_expires: subscribe.group_expires,
			group_order: subscribe.group_order,
		};

		let mut track = self.get_track(track).await?;

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

	#[tracing::instrument("fetch", skip_all, err, fields(track = ?fetch.path, group = fetch.group, offset = fetch.offset))]
	async fn serve_fetch(&mut self, _stream: &mut Stream, fetch: message::Fetch) -> Result<(), Error> {
		let track = Track {
			path: fetch.path,
			priority: fetch.priority,
			..Default::default()
		};

		let track = self.get_track(track).await?;
		let _group = track.get_group(fetch.group)?;

		unimplemented!("TODO fetch");
	}

	pub async fn recv_info(&mut self, stream: &mut Stream) -> Result<(), Error> {
		let info = stream.reader.decode().await?;
		self.serve_info(stream, info).await
	}

	#[tracing::instrument("info", skip_all, err, fields(track = ?info.path))]
	async fn serve_info(&mut self, stream: &mut Stream, info: message::InfoRequest) -> Result<(), Error> {
		let track = Track {
			path: info.path,
			..Default::default()
		};
		let track = self.get_track(track).await?;

		let info = message::Info {
			group_latest: track.latest_group(),
			track_priority: track.priority,
			group_expires: track.group_expires,
			group_order: track.group_order,
		};

		stream.writer.encode(&info).await?;

		Ok(())
	}

	async fn get_track(&self, track: Track) -> Result<TrackConsumer, Error> {
		if let Some(track) = self.tracks.lock().get(&track.path) {
			return Ok(track.clone());
		}

		if let Some(router) = &self.router {
			return router.subscribe(track).await;
		}

		Err(Error::NotFound)
	}
}
