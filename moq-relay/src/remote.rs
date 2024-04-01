use std::collections::HashMap;

use std::collections::VecDeque;
use std::fmt;
use std::ops;
use std::sync::Arc;
use std::sync::Weak;

use futures::stream::FuturesUnordered;
use futures::FutureExt;
use futures::StreamExt;
use moq_transport::serve::{ServeError, Track, TrackReader, TrackWriter};
use moq_transport::util::State;
use url::Url;

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
	lookup: HashMap<Url, RemoteConsumer>,
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
					let url = remote.url.clone();

					tasks.push(async move {
						let info = remote.info.clone();

						log::warn!("serving remote: {:?}", info);
						if let Err(err) = remote.run().await {
							log::warn!("failed serving remote: {:?}, error: {}", info, err);
						}

						url
					});
				}
				res = tasks.next(), if !tasks.is_empty() => {
					let url = res.unwrap();
					self.state.lock_mut().ok_or(ServeError::Done)?.lookup.remove(&url);
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
	pub info: Arc<Remotes>,
	state: State<RemotesState>,
}

impl RemotesConsumer {
	fn new(info: Arc<Remotes>, state: State<RemotesState>) -> Self {
		Self { info, state }
	}

	pub async fn route(&self, namespace: &str) -> Result<Option<RemoteConsumer>, RelayError> {
		// Always fetch the origin instead of using the (potentially invalid) cache.
		let origin = match self.api.get_origin(namespace).await.map_err(Arc::new)? {
			None => return Ok(None),
			Some(origin) => origin,
		};

		let state = self.state.lock();
		if let Some(remote) = state.lookup.get(&origin.url).cloned() {
			return Ok(Some(remote));
		}

		let mut state = state.into_mut().ok_or(ServeError::Done)?;

		let remote = Remote {
			url: origin.url.clone(),
			remotes: self.info.clone(),
		};

		let (writer, reader) = remote.produce();
		state.requested.push_back(writer);

		state.lookup.insert(origin.url, reader.clone());

		Ok(Some(reader))
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
	pub url: Url,
}

impl fmt::Debug for Remote {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Remote").field("url", &self.url.to_string()).finish()
	}
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
	tracks: HashMap<(String, String), RemoteTrackWeak>,
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
		// TODO reuse QUIC and MoQ sessions
		let session = webtransport_quinn::connect(&self.quic, &self.url).await?;
		let (session, mut subscriber) = moq_transport::Subscriber::connect(session).await?;

		// Run the session
		let mut session = session.run().boxed();
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				track = self.next() => {
					let track = track?;
					let info = track.info.clone();

					let subscribe = match subscriber.subscribe(track) {
						Ok(subscribe) => subscribe,
						Err(err) => {
							log::warn!("failed subscribing: {:?}, error: {}", info, err);
							continue
						}
					};

					tasks.push(async move {
						if let Err(err) = subscribe.closed().await {
							log::warn!("failed serving track: {:?}, error: {}", info, err);
						}
					});
				}
				_ = tasks.next(), if !tasks.is_empty() => {},

				res = &mut session => res?,
			}
		}
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
	pub fn subscribe(&self, namespace: &str, name: &str) -> Result<RemoteTrackReader, RelayError> {
		let key = (namespace.to_string(), name.to_string());
		let state = self.state.lock();
		if let Some(track) = state.tracks.get(&key) {
			if let Some(track) = track.upgrade() {
				return Ok(track);
			}
		}

		let mut state = state.into_mut().ok_or(ServeError::Done)?;

		let (writer, reader) = Track::new(namespace, name).produce();
		let reader = RemoteTrackReader::new(reader, self.state.clone());

		// Insert the track into our Map so we deduplicate future requests.
		state.tracks.insert(key, reader.downgrade());
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

#[derive(Clone)]
pub struct RemoteTrackReader {
	pub reader: TrackReader,
	drop: Arc<RemoteTrackDrop>,
}

impl RemoteTrackReader {
	fn new(reader: TrackReader, parent: State<RemoteState>) -> Self {
		let drop = Arc::new(RemoteTrackDrop {
			parent,
			key: (reader.namespace.clone(), reader.name.clone()),
		});

		Self { reader, drop }
	}

	fn downgrade(&self) -> RemoteTrackWeak {
		RemoteTrackWeak {
			reader: self.reader.clone(),
			drop: Arc::downgrade(&self.drop),
		}
	}
}

impl ops::Deref for RemoteTrackReader {
	type Target = TrackReader;

	fn deref(&self) -> &Self::Target {
		&self.reader
	}
}

impl ops::DerefMut for RemoteTrackReader {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.reader
	}
}

struct RemoteTrackWeak {
	reader: TrackReader,
	drop: Weak<RemoteTrackDrop>,
}

impl RemoteTrackWeak {
	fn upgrade(&self) -> Option<RemoteTrackReader> {
		Some(RemoteTrackReader {
			reader: self.reader.clone(),
			drop: self.drop.upgrade()?,
		})
	}
}

struct RemoteTrackDrop {
	parent: State<RemoteState>,
	key: (String, String),
}

impl Drop for RemoteTrackDrop {
	fn drop(&mut self) {
		if let Some(mut parent) = self.parent.lock_mut() {
			parent.tracks.remove(&self.key);
		}
	}
}
