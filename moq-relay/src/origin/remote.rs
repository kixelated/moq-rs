use std::collections::HashMap;

use std::collections::VecDeque;
use std::ops;
use std::sync::Arc;

use futures::stream::FuturesUnordered;
use futures::FutureExt;
use futures::StreamExt;
use moq_transport::serve::{ServeError, Track, TrackReader, TrackWriter};
use moq_transport::util::State;

use crate::RelayError;

pub struct Remotes {
	/// The client we use to fetch/store origin information.
	pub api: moq_api::Client,

	// A QUIC endpoint we'll use to fetch from other origins.
	pub quic: quinn::Endpoint,
}

impl Remotes {
	pub fn produce(self) -> (RemotesProducer, RemotesConsumer) {
		let (send, recv) = State::default();
		let info = Arc::new(self);

		let producer = RemotesProducer::new(info.clone(), send);
		let consumer = RemotesConsumer::new(info, recv);

		(producer, consumer)
	}
}

#[derive(Default)]
struct RemotesState {
	lookup: HashMap<String, RemoteConsumer>,
	requested: VecDeque<RemoteProducer>,
}

// Clone for convenience, but there should only be one instance of this
#[derive(Clone)]
pub struct RemotesProducer {
	info: Arc<Remotes>,
	state: State<RemotesState>,
}

impl RemotesProducer {
	fn new(info: Arc<Remotes>, state: State<RemotesState>) -> Self {
		Self { info, state }
	}

	async fn next(&mut self) -> Result<RemoteProducer, RelayError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if !state.requested.is_empty() {
					let mut state = state.into_mut().ok_or(ServeError::Done)?;
					return Ok(state.requested.pop_front().unwrap());
				}

				state.modified().ok_or(ServeError::Done)?
			};

			notify.await
		}
	}

	pub async fn run(mut self) -> Result<(), RelayError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				remote = self.next() => {
					let remote = remote?;
					let namespace = remote.info.namespace.clone();

					tasks.push(async move {
						if let Err(err) = remote.run().await {
							log::warn!("failed serving remote: err={}", err);
						}

						namespace
					});
				}
				namespace = tasks.select_next_some() => {
					self.state.lock_mut().ok_or(ServeError::Done)?.lookup.remove(&namespace);
				},
			}
		}
	}
}

impl ops::Deref for RemotesProducer {
	type Target = Remotes;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

#[derive(Clone)]
pub struct RemotesConsumer {
	info: Arc<Remotes>,
	state: State<RemotesState>,
}

impl RemotesConsumer {
	fn new(info: Arc<Remotes>, state: State<RemotesState>) -> Self {
		Self { info, state }
	}

	pub fn fetch(&self, namespace: &str) -> Result<RemoteConsumer, RelayError> {
		let state = self.state.lock();
		if let Some(remote) = state.lookup.get(namespace).cloned() {
			return Ok(remote);
		}

		let mut state = state.into_mut().ok_or(ServeError::Done)?;
		let remote = Remote {
			namespace: namespace.to_string(),
			remotes: self.info.clone(),
		};

		let (writer, reader) = remote.produce();
		state.requested.push_back(writer);

		Ok(reader)
	}
}

impl ops::Deref for RemotesConsumer {
	type Target = Remotes;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

pub struct Remote {
	pub remotes: Arc<Remotes>,
	pub namespace: String,
}

impl ops::Deref for Remote {
	type Target = Remotes;

	fn deref(&self) -> &Self::Target {
		&self.remotes
	}
}

impl Remote {
	/// Create a new broadcast.
	pub fn produce(self) -> (RemoteProducer, RemoteConsumer) {
		let (send, recv) = State::default();
		let info = Arc::new(self);

		let consumer = RemoteConsumer::new(info.clone(), recv);
		let producer = RemoteProducer::new(info, send);

		(producer, consumer)
	}
}

struct RemoteState {
	tracks: HashMap<String, TrackReader>,
	requested: VecDeque<TrackWriter>,
	closed: Result<(), RelayError>,
}

impl Default for RemoteState {
	fn default() -> Self {
		Self {
			tracks: HashMap::new(),
			requested: VecDeque::new(),
			closed: Ok(()),
		}
	}
}

pub struct RemoteProducer {
	pub info: Arc<Remote>,
	state: State<RemoteState>,
}

impl RemoteProducer {
	fn new(info: Arc<Remote>, state: State<RemoteState>) -> Self {
		Self { info, state }
	}

	pub async fn run(mut self) -> Result<(), RelayError> {
		if let Err(err) = self.run_inner().await {
			self.state.lock_mut().ok_or(ServeError::Done)?.closed = Err(err.clone());
			return Err(err);
		}

		Ok(())
	}

	pub async fn run_inner(&mut self) -> Result<(), RelayError> {
		let (session, mut subscriber) = self.connect().await?;

		// Run the session
		let mut session = session.run().boxed();
		let mut tracks = FuturesUnordered::new();

		loop {
			tokio::select! {
				track = self.next() => {
					let track = track?;
					let info = track.info.clone();

					let subscribe = match subscriber.subscribe(track) {
						Ok(subscribe) => subscribe,
						Err(err) => {
							log::warn!("failed subscribing: track={:?} err={:?}", info, err);
							continue
						}
					};

					tracks.push(async move {
						if let Err(err) = subscribe.run().await {
							log::warn!("failed serving track: track={:?} err={:?}", info, err);
						}
					});
				}
				_ = tracks.select_next_some() => {},
				res = &mut session => res?,
			}
		}
	}

	async fn connect(
		&mut self,
	) -> Result<
		(
			moq_transport::Session<webtransport_quinn::Session>,
			moq_transport::Subscriber<webtransport_quinn::Session>,
		),
		RelayError,
	> {
		let remotes = &self.info.remotes;

		let origin = remotes
			.api
			.get_origin(&self.info.namespace)
			.await
			.map_err(Arc::new)?
			.ok_or(ServeError::NotFound)?;

		// TODO reuse QUIC and MoQ sessions
		let session = webtransport_quinn::connect(&remotes.quic, &origin.url).await?;
		let (session, subscriber) = moq_transport::Subscriber::connect(session).await?;

		Ok((session, subscriber))
	}

	/// Block until the next track requested by a consumer.
	async fn next(&self) -> Result<TrackWriter, RelayError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if !state.requested.is_empty() {
					let mut state = state.into_mut().ok_or(ServeError::Done)?;
					return Ok(state.requested.pop_front().unwrap());
				}

				state.modified().ok_or(ServeError::Done)?
			};

			notify.await
		}
	}
}

impl ops::Deref for RemoteProducer {
	type Target = Remote;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

#[derive(Clone)]
pub struct RemoteConsumer {
	pub info: Arc<Remote>,
	state: State<RemoteState>,
}

impl RemoteConsumer {
	fn new(info: Arc<Remote>, state: State<RemoteState>) -> Self {
		Self { info, state }
	}

	/// Request a track from the broadcast.
	pub fn subscribe(&self, name: &str) -> Result<TrackReader, RelayError> {
		let state = self.state.lock();
		if let Some(track) = state.tracks.get(name).cloned() {
			return Ok(track);
		}

		let mut state = state.into_mut().ok_or(ServeError::Done)?;

		let (writer, reader) = Track::new(&self.info.namespace, name).produce();
		state.requested.push_back(writer);

		Ok(reader)
	}
}

impl ops::Deref for RemoteConsumer {
	type Target = Remote;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
