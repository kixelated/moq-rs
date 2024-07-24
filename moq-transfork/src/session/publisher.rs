use std::collections::{hash_map, HashMap};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	message,
	model::{Broadcast, BroadcastReader, Closed, GroupReader, RouterReader, Track, TrackReader},
	runtime::{self, Lock},
};

use super::{OrClose, Session, SessionError, Stream, Writer};

#[derive(Clone)]
pub struct Publisher {
	session: Session,

	// Used to route incoming subscriptions
	broadcasts: Lock<HashMap<String, BroadcastReader>>,
	router: Lock<Option<RouterReader<Broadcast>>>,
}

impl Publisher {
	pub(crate) fn new(session: Session) -> Self {
		Self {
			session,
			broadcasts: Default::default(),
			router: Default::default(),
		}
	}

	/// Announce a broadcast and serve tracks using the returned [BroadcastWriter].
	#[tracing::instrument("announce", skip_all, err, fields(broadcast = broadcast.name))]
	pub async fn announce(&mut self, broadcast: BroadcastReader) -> Result<(), SessionError> {
		let announce = self.init_announce(broadcast)?;

		let mut stream = self.session.open(message::Stream::Announce).await?;
		self.start_announce(&mut stream, &announce)
			.await
			.or_close(&mut stream)?;

		runtime::spawn(async move {
			Self::run_announce(stream, announce).await.ok();
		});

		Ok(())
	}

	fn init_announce(&mut self, broadcast: BroadcastReader) -> Result<Announce, SessionError> {
		match self.broadcasts.lock().entry(broadcast.name.clone()) {
			hash_map::Entry::Occupied(_) => return Err(Closed::Duplicate.into()),
			hash_map::Entry::Vacant(entry) => entry.insert(broadcast.clone()),
		};

		Ok(Announce {
			broadcast,
			broadcasts: self.broadcasts.clone(),
		})
	}

	async fn start_announce(&mut self, stream: &mut Stream, announce: &Announce) -> Result<(), SessionError> {
		let announce = message::Announce {
			broadcast: announce.broadcast.name.clone(),
		};

		stream.writer.encode(&announce).await?;

		let _ok = stream.reader.decode::<message::AnnounceOk>().await?;
		tracing::info!("ok");

		Ok(())
	}

	async fn run_announce(mut stream: Stream, announce: Announce) -> Result<(), SessionError> {
		tokio::select! {
			// Keep the stream open until the broadcast is closed
			res = stream.reader.closed() => res.map_err(SessionError::from),
			res = announce.broadcast.closed() => res.map_err(SessionError::from),
		}
		.or_close(&mut stream)
	}

	// Optionally send any requests for unknown broadcasts to the router
	pub fn route(&mut self, router: RouterReader<Broadcast>) {
		*self.router.lock() = Some(router);
	}

	async fn subscribe(&mut self, broadcast: Broadcast, track: Track) -> Result<TrackReader, Closed> {
		let reader = self.broadcasts.lock().get(&broadcast.name).cloned();
		if let Some(mut broadcast) = reader {
			return broadcast.subscribe(track).await;
		}

		let router = self.router.lock().clone();
		if let Some(router) = router {
			let mut reader = router.subscribe(broadcast).await?;
			return reader.subscribe(track).await;
		}

		Err(Closed::Unknown)
	}

	pub(super) async fn recv_subscribe(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let subscribe = stream.reader.decode().await?;
		self.serve_subscribe(stream, subscribe).await
	}

	#[tracing::instrument("subscribed", skip_all, err, fields(broadcast = subscribe.broadcast, track = subscribe.track, id = subscribe.id))]
	async fn serve_subscribe(
		&mut self,
		stream: &mut Stream,
		subscribe: message::Subscribe,
	) -> Result<(), SessionError> {
		let broadcast = Broadcast::new(subscribe.broadcast);
		let track = Track::new(subscribe.track, subscribe.priority).build();
		let mut track = self.subscribe(broadcast.clone(), track).await?;

		let info = message::Info {
			group_latest: track.latest(),
			group_expires: track.group_expires,
			group_order: track.group_order,
			track_priority: track.priority,
		};

		stream.writer.encode(&info).await?;

		tracing::info!("ok");

		let mut tasks = FuturesUnordered::new();
		let mut fin = false;

		loop {
			tokio::select! {
				res = track.next(), if !fin => {
					let mut group = match res? {
						Some(group) => group,
						None => {
							fin = true;
							continue;
						},
					};

					let session = self.session.clone();

					tasks.push(async move {
						let res = Self::serve_group(session, subscribe.id, &mut group).await;
						(group, res)
					});
				},
				res = stream.reader.decode_maybe::<message::SubscribeUpdate>() => {
					match res? {
						Some(_update) => {
							// TODO use it
						},
						None => return Ok(()),
					}
				},
				res = tasks.next(), if !tasks.is_empty() => {
					let (group, res) = res.unwrap();

					if let Err(err) = res {
						let drop = message::GroupDrop {
							sequence: group.sequence,
							count: 0,
							code: err.code(),
						};

						stream.writer.encode(&drop).await?;
					}
				},
			}
		}
	}

	#[tracing::instrument("data", skip_all, err, fields(group = group.sequence))]
	pub async fn serve_group(
		mut session: Session,
		subscribe: u64,
		group: &mut GroupReader,
	) -> Result<(), SessionError> {
		let mut stream = session.open_uni(message::StreamUni::Group).await?;

		Self::serve_group_inner(subscribe, group, &mut stream)
			.await
			.or_close(&mut stream)
	}

	pub async fn serve_group_inner(
		subscribe: u64,
		group: &mut GroupReader,
		stream: &mut Writer,
	) -> Result<(), SessionError> {
		let msg = message::Group {
			subscribe,
			sequence: group.sequence,
		};

		stream.encode(&msg).await?;

		// TODO abort if the subscription is closed
		let mut size = 0;

		while let Some(chunk) = group.read().await? {
			size += chunk.len();
			stream.write(&chunk).await?;
		}

		// TODO block until all bytes have been acknowledged so we can still reset
		// writer.finish().await?;

		tracing::debug!(size);

		Ok(())
	}

	pub(super) async fn recv_datagrams(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let subscribe = stream.reader.decode().await?;
		self.serve_datagrams(stream, subscribe).await
	}

	#[tracing::instrument("datagrams", skip_all, err, fields(broadcast = subscribe.broadcast, track = subscribe.track, subscribe = subscribe.id))]
	async fn serve_datagrams(
		&mut self,
		stream: &mut Stream,
		subscribe: message::Subscribe,
	) -> Result<(), SessionError> {
		todo!("datagrams");
	}

	pub(super) async fn recv_fetch(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let fetch = stream.reader.decode().await?;
		self.serve_fetch(stream, fetch).await
	}

	#[tracing::instrument("fetch", skip_all, err, fields(broadcast = fetch.broadcast, track = fetch.track, group = fetch.group, offset = fetch.offset))]
	async fn serve_fetch(&mut self, stream: &mut Stream, fetch: message::Fetch) -> Result<(), SessionError> {
		let broadcast = Broadcast::new(fetch.broadcast);
		let track = Track::new(fetch.track, fetch.priority).build();
		let track = self.subscribe(broadcast, track).await?;
		let group = track.get(fetch.group)?;

		unimplemented!("TODO fetch");

		/*
		group.skip(fetch.offset);

		while let Some(chunk) = group.read().await {
			let chunk = chunk?;
			writer.write(&chunk).await?;
		}
		*/

		Ok(())
	}

	pub(super) async fn recv_info(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let info = stream.reader.decode().await?;
		self.serve_info(stream, info).await
	}

	#[tracing::instrument("track", skip_all, err, fields(broadcast = info.broadcast, track = info.track))]
	async fn serve_info(&mut self, stream: &mut Stream, info: message::InfoRequest) -> Result<(), SessionError> {
		let broadcast = Broadcast::new(info.broadcast);
		let track = Track::new(info.track, 0).build();
		let track = self.subscribe(broadcast, track).await?;

		let info = message::Info {
			group_latest: track.latest(),
			track_priority: track.priority,
			group_expires: track.group_expires,
			group_order: track.group_order,
		};

		stream.writer.encode(&info).await?;

		Ok(())
	}

	pub async fn closed(&self) -> Result<(), SessionError> {
		self.session.closed().await
	}
}

struct Announce {
	pub broadcast: BroadcastReader,
	broadcasts: Lock<HashMap<String, BroadcastReader>>,
}

impl Drop for Announce {
	fn drop(&mut self) {
		self.broadcasts.lock().remove(&self.broadcast.name);
	}
}
