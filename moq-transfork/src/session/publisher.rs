use std::collections::{hash_map, HashMap};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	message,
	model::{GroupConsumer, Track, TrackConsumer},
	util::{spawn, FuturesExt, Lock, OrClose},
	Announced, AnnouncedConsumer, AnnouncedProducer, Error, Path, RouterConsumer,
};

use super::{Stream, Writer};

#[derive(Clone)]
pub(super) struct Publisher {
	session: web_transport::Session,
	announced: AnnouncedProducer,
	tracks: Lock<HashMap<Path, TrackConsumer>>,
	router: Lock<Option<RouterConsumer>>,
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

	/// Publish a track.
	#[tracing::instrument("publish", skip_all, err, fields(?track))]
	pub fn publish(&mut self, track: TrackConsumer) -> Result<(), Error> {
		if !self.announced.announce(track.path.clone()) {
			return Err(Error::Duplicate);
		}

		match self.tracks.lock().entry(track.path.clone()) {
			hash_map::Entry::Occupied(_) => return Err(Error::Duplicate),
			hash_map::Entry::Vacant(entry) => entry.insert(track.clone()),
		};

		let mut this = self.clone();

		spawn(async move {
			tokio::select! {
				_ = track.closed() => (),
				_ = this.session.closed() => (),
			}
			this.tracks.lock().remove(&track.path);
			this.announced.unannounce(&track.path);
		});

		Ok(())
	}

	/// Announce the given tracks.
	/// This is an advanced API for producing tracks dynamically.
	/// NOTE: You may want to call [Self::route] to process any subscriptions for these paths.
	pub fn announce(&mut self, mut announced: AnnouncedConsumer) {
		let mut downstream = self.announced.clone();

		spawn(async move {
			while let Some(announced) = announced.next().await {
				match announced {
					Announced::Active(path) => downstream.announce(path.clone()),
					Announced::Ended(path) => downstream.unannounce(&path),

					// Indicate that we're caught up to live.
					Announced::Live => downstream.live(),
				};
			}
		});
	}

	/// Optionally support requests for arbitrary paths using the provided router.
	/// This is an advanced API for producing tracks dynamically.
	/// NOTE: You may want to call [Self::announce] to advertise these paths.
	pub fn route(&mut self, router: RouterConsumer) {
		// TODO support multiple routers?
		self.router.lock().replace(router);
	}

	pub async fn recv_announce(&mut self, stream: &mut Stream) -> Result<(), Error> {
		let interest = stream.reader.decode::<message::AnnouncePlease>().await?;
		let prefix = interest.prefix;
		tracing::debug!(?prefix, "announce interest");

		let mut announced = self.announced.subscribe_prefix(prefix.clone());

		// Flush any synchronously announced paths
		while let Some(announced) = announced.next().await {
			match announced {
				Announced::Active(path) => {
					tracing::debug!(?path, "announce");
					stream
						.writer
						.encode(&message::Announce::Active { suffix: path })
						.await?;
				}
				Announced::Ended(path) => {
					tracing::debug!(?path, "unannounce");
					stream.writer.encode(&message::Announce::Ended { suffix: path }).await?;
				}
				Announced::Live => {
					// Indicate that we're caught up to live.
					tracing::debug!("live");
					stream.writer.encode(&message::Announce::Live).await?;
				}
			}
		}

		Ok(())
	}

	pub async fn recv_subscribe(&mut self, stream: &mut Stream) -> Result<(), Error> {
		let subscribe = stream.reader.decode().await?;
		self.serve_subscribe(stream, subscribe).await
	}

	#[tracing::instrument("subscribed", skip_all, err, fields(track = ?subscribe.path, id = subscribe.id))]
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

		tracing::info!(?info);

		stream.writer.encode(&info).await?;

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
						tracing::warn!(?err, subscribe = ?subscribe.id, group = group.sequence, "dropped");

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

	#[tracing::instrument("group", skip_all, fields(?subscribe, sequence = group.sequence))]
	pub async fn serve_group(
		mut session: web_transport::Session,
		subscribe: u64,
		group: &mut GroupConsumer,
	) -> Result<(), Error> {
		let mut stream = Writer::open(&mut session, message::DataType::Group).await?;
		tracing::trace!("serving");

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

		let router = self.router.lock().clone();
		match router {
			Some(router) => router.subscribe(track).await,
			None => Err(Error::NotFound),
		}
	}
}
