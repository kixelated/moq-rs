use std::collections::{hash_map, HashMap};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	message,
	model::{GroupConsumer, Track, TrackConsumer},
	Announced, AnnouncedConsumer, AnnouncedProducer, Error, RouterConsumer,
};

use moq_async::{spawn, FuturesExt, Lock, OrClose};

use super::{Stream, Writer};

#[derive(Clone)]
pub(super) struct Publisher {
	session: web_transport::Session,
	announced: AnnouncedProducer,
	tracks: Lock<HashMap<String, TrackConsumer>>,
	router: Lock<Option<RouterConsumer>>,
}

impl Publisher {
	pub fn new(session: web_transport::Session) -> Self {
		// We start the publisher in live mode because we're producing content.
		let mut announced = AnnouncedProducer::new();
		announced.live();

		Self {
			session,
			announced,
			tracks: Default::default(),
			router: Default::default(),
		}
	}

	/// Publish a track.
	#[tracing::instrument("publish", skip_all, err, fields(?track))]
	pub fn publish(&mut self, track: TrackConsumer) -> Result<(), Error> {
		if !self.announced.announce(&track.path) {
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
	///
	/// This is an advanced API for producing tracks dynamically.
	/// NOTE: You may want to call [Self::route] to process any subscriptions for these paths.
	/// [crate::AnnouncedConsumer] will automatically unannounce if the [crate::AnnouncedProducer] is dropped.
	pub fn announce(&mut self, mut upstream: AnnouncedConsumer) {
		let mut downstream = self.announced.clone();

		spawn(async move {
			while let Some(announced) = upstream.next().await {
				match announced {
					Announced::Active(m) => downstream.announce(m.full()),
					Announced::Ended(m) => downstream.unannounce(m.full()),

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
		let filter = interest.filter;
		tracing::debug!(?filter, "announce interest");

		let mut announced = self.announced.subscribe(filter);

		// Flush any synchronously announced paths
		while let Some(announced) = announced.next().await {
			match announced {
				Announced::Active(m) => {
					let msg = message::Announce::Active(m.capture().to_string());
					stream.writer.encode(&msg).await?;
				}
				Announced::Ended(m) => {
					let msg = message::Announce::Ended(m.capture().to_string());
					stream.writer.encode(&msg).await?;
				}
				Announced::Live => {
					// Indicate that we're caught up to live.
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

	#[tracing::instrument("publishing", skip_all, err, fields(track = ?subscribe.path, id = subscribe.id))]
	async fn serve_subscribe(&mut self, stream: &mut Stream, subscribe: message::Subscribe) -> Result<(), Error> {
		let track = Track {
			path: subscribe.path,
			priority: subscribe.priority,
			order: subscribe.group_order,
		};

		let mut track = self.get_track(track).await?;

		let info = message::Info {
			group_latest: track.latest_group(),
			group_order: track.order,
			track_priority: track.priority,
		};

		tracing::info!(?info, "active");

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
			group_order: track.order,
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
