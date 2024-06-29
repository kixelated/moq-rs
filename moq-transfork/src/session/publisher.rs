use std::{
	collections::{hash_map, HashMap},
	fmt,
	sync::{Arc, Mutex},
};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	coding::Stream, message, setup, util::Queue, BroadcastReader, Closed, Track, TrackReader, Unknown, UnknownReader,
	UnknownWriter,
};

use super::{Announce, Session, SessionError, Subscribed};

#[derive(Clone)]
pub struct Publisher {
	webtransport: web_transport::Session,

	// Used to route incoming subscriptions
	broadcasts: Arc<Mutex<HashMap<String, BroadcastReader>>>,

	announced: Queue<Announce>,
	unknown: Option<UnknownReader>,
}

impl Publisher {
	pub(crate) fn new(webtransport: web_transport::Session) -> Self {
		Self {
			webtransport,
			broadcasts: Default::default(),
			announced: Default::default(),
			unknown: None,
		}
	}

	pub async fn accept(session: web_transport::Session) -> Result<(Session, Publisher), SessionError> {
		let (session, publisher, _) = Session::accept_role(session, setup::Role::Publisher).await?;
		Ok((session, publisher.unwrap()))
	}

	pub async fn connect(session: web_transport::Session) -> Result<(Session, Publisher), SessionError> {
		let (session, publisher, _) = Session::connect_role(session, setup::Role::Publisher).await?;
		Ok((session, publisher.unwrap()))
	}

	/// Announce a broadcast and serve tracks using the returned [BroadcastWriter].
	pub async fn announce(&mut self, broadcast: BroadcastReader) -> Result<(), SessionError> {
		match self.broadcasts.lock().unwrap().entry(broadcast.name.clone()) {
			hash_map::Entry::Occupied(_) => return Err(Closed::Duplicate.into()),
			hash_map::Entry::Vacant(entry) => entry.insert(broadcast.clone()),
		};

		let stream = Stream::open(&mut self.webtransport, message::Control::Announce).await?;
		let mut announce = Announce::new(stream, broadcast);
		announce.start().await?;

		if self.announced.push(announce).is_err() {
			return Err(SessionError::Internal);
		}

		Ok(())
	}

	/// Route any unknown subscriptions to the provided [UnknownWriter].
	///
	/// If this is not called, any uknonwn subscriptions will be rejected with [ServeError::NotFound].
	/// This may be called multiple times, but only the last one will be used.
	pub fn unknown(&mut self) -> UnknownWriter {
		let (writer, reader) = Unknown::produce();
		self.unknown = Some(reader);
		writer
	}

	pub(super) async fn run(self) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(mut announce) = self.announced.pop() => {
					tasks.push(async move {
						announce.run().await;
						announce
					});
				},
				res = tasks.next(), if !tasks.is_empty() => {
					let announce = res.unwrap();
					self.broadcasts.lock().unwrap().remove(announce.id());
				},
			}
		}
	}

	async fn subscribe(&mut self, track: Track) -> Result<TrackReader, Closed> {
		let broadcast = self.broadcasts.lock().unwrap().get(&track.broadcast).cloned();
		if let Some(mut broadcast) = broadcast {
			return broadcast.subscribe(track).await;
		}

		if let Some(unknown) = self.unknown.as_mut() {
			return unknown.subscribe(track).await;
		}

		Err(Closed::UnknownBroadcast)
	}

	#[tracing::instrument("serving subscribe", skip(self), fields(session = self.webtransport.id(), stream = stream.id()))]
	pub(super) async fn recv_subscribe(&mut self, mut stream: Stream) {
		if let Err(err) = self.recv_subscribe_inner(&mut stream).await {
			tracing::warn!(?err);
			stream.writer.reset(err.code());
		}
	}

	async fn recv_subscribe_inner(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let request: message::Subscribe = stream.reader.decode().await?;
		tracing::info!(?request);

		let track = Track::new(&request.broadcast, &request.track, request.priority).build();
		let track = self.subscribe(track).await?;

		let info = message::Info {
			group_latest: track.latest(),
			group_expires: track.group_expires,
			group_order: track.group_order,
			track_priority: track.priority,
		};
		tracing::info!(?info);

		stream.writer.encode(&info).await?;

		let subscribed = Subscribed::new(self.webtransport.clone(), request, track);
		subscribed.run(stream).await
	}

	#[tracing::instrument("serving datagrams", skip(self), fields(session = self.webtransport.id(), stream = stream.id()))]
	pub(super) async fn recv_datagrams(&mut self, mut stream: Stream) {
		if let Err(err) = self.recv_datagrams_inner(&mut stream).await {
			tracing::warn!(?err);
			stream.writer.reset(err.code());
		}
	}

	async fn recv_datagrams_inner(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let request: message::Subscribe = stream.reader.decode().await?;
		tracing::info!(?request);

		todo!("datagrams");
	}

	#[tracing::instrument("serving fetch", skip(self), fields(session = self.webtransport.id(), stream = stream.id()))]
	pub(super) async fn recv_fetch(&mut self, mut stream: Stream) {
		if let Err(err) = self.recv_fetch_inner(&mut stream).await {
			tracing::warn!(?err);
			stream.writer.reset(err.code());
		}
	}

	async fn recv_fetch_inner(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let request: message::Fetch = stream.reader.decode().await?;
		tracing::info!(?request);

		let track = Track::new(&request.broadcast, &request.track, request.priority).build();
		let track = self.subscribe(track).await?;
		let group = track.get(request.group)?;

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

	#[tracing::instrument("serving info", skip(self), fields(session = self.webtransport.id(), stream = stream.id()))]
	pub(super) async fn recv_info(&mut self, mut stream: Stream) {
		if let Err(err) = self.recv_info_inner(&mut stream).await {
			tracing::warn!(?err);
			stream.writer.reset(err.code());
		}
	}

	async fn recv_info_inner(&mut self, stream: &mut Stream) -> Result<(), SessionError> {
		let request: message::InfoRequest = stream.reader.decode().await?;
		tracing::info!(?request);

		let track = Track::new(&request.broadcast, &request.track, 0).build();
		let track = self.subscribe(track).await?;

		let response = message::Info {
			group_latest: track.latest(),
			track_priority: track.priority,
			group_expires: track.group_expires,
			group_order: track.group_order,
		};
		tracing::info!(?response);

		stream.writer.encode(&response).await?;

		Ok(())
	}
}
