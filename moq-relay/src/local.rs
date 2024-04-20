use std::collections::hash_map;
use std::collections::HashMap;

use std::collections::VecDeque;
use std::fmt;
use std::ops;
use std::sync::Arc;
use std::sync::Weak;

use moq_transport::serve::{self, ServeError, TrackReader, TrackWriter};
use moq_transport::util::State;
use tokio::time;
use url::Url;

use crate::RelayError;

pub struct Locals {
	pub api: Option<moq_api::Client>,
	pub node: Option<Url>,
}

impl Locals {
	pub fn produce(self) -> (LocalsProducer, LocalsConsumer) {
		let (send, recv) = State::init();
		let info = Arc::new(self);

		let producer = LocalsProducer::new(info.clone(), send);
		let consumer = LocalsConsumer::new(info, recv);

		(producer, consumer)
	}
}

#[derive(Default)]
struct LocalsState {
	lookup: HashMap<String, LocalConsumer>,
}

#[derive(Clone)]
pub struct LocalsProducer {
	info: Arc<Locals>,
	state: State<LocalsState>,
}

impl LocalsProducer {
	fn new(info: Arc<Locals>, state: State<LocalsState>) -> Self {
		Self { info, state }
	}

	pub async fn announce(&mut self, namespace: &str) -> Result<LocalProducer, RelayError> {
		let (mut writer, reader) = Local {
			namespace: namespace.to_string(),
			locals: self.info.clone(),
		}
		.produce(self.clone());

		// Try to insert with the API.
		writer.register().await?;

		let mut state = self.state.lock_mut().unwrap();
		match state.lookup.entry(namespace.to_string()) {
			hash_map::Entry::Vacant(entry) => entry.insert(reader),
			hash_map::Entry::Occupied(_) => return Err(ServeError::Duplicate.into()),
		};

		Ok(writer)
	}

	async fn unannounce(&mut self, namespace: &str) -> Result<(), RelayError> {
		if let Some(mut state) = self.state.lock_mut() {
			state.lookup.remove(namespace).ok_or(ServeError::NotFound)?;
		}

		if let Some(api) = self.api.as_ref() {
			api.delete_origin(namespace).await.map_err(Arc::new)?;
		}

		Ok(())
	}
}

impl ops::Deref for LocalsProducer {
	type Target = Locals;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

#[derive(Clone)]
pub struct LocalsConsumer {
	pub info: Arc<Locals>,
	state: State<LocalsState>,
}

impl LocalsConsumer {
	fn new(info: Arc<Locals>, state: State<LocalsState>) -> Self {
		Self { info, state }
	}

	pub fn route(&self, namespace: &str) -> Option<LocalConsumer> {
		let state = self.state.lock();
		state.lookup.get(namespace).cloned()
	}
}

impl ops::Deref for LocalsConsumer {
	type Target = Locals;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

pub struct Local {
	pub namespace: String,
	pub locals: Arc<Locals>,
}

impl Local {
	/// Create a new broadcast.
	fn produce(self, parent: LocalsProducer) -> (LocalProducer, LocalConsumer) {
		let (send, recv) = State::init();
		let info = Arc::new(self);

		let writer = LocalProducer::new(info.clone(), send, parent);
		let reader = LocalConsumer::new(info, recv);

		(writer, reader)
	}
}

impl fmt::Debug for Local {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Local").field("namespace", &self.namespace).finish()
	}
}

#[derive(Default)]
struct LocalState {
	tracks: HashMap<String, LocalTrackWeak>,
	requested: VecDeque<TrackWriter>,
}

impl Drop for LocalState {
	fn drop(&mut self) {
		for track in self.requested.drain(..) {
			track.close(ServeError::NotFound).ok();
		}
	}
}

/// Publish new tracks for a broadcast by name.
pub struct LocalProducer {
	pub info: Arc<Local>,
	state: State<LocalState>,

	parent: LocalsProducer,
	refresh: tokio::time::Interval,
}

impl LocalProducer {
	fn new(info: Arc<Local>, state: State<LocalState>, parent: LocalsProducer) -> Self {
		let delay = time::Duration::from_secs(300);
		let mut refresh = time::interval(delay);
		refresh.reset_after(delay); // Skip the first tick

		Self {
			info,
			state,
			refresh,
			parent,
		}
	}

	/// Block until the next track requested by a reader.
	pub async fn requested(&mut self) -> Result<Option<serve::TrackWriter>, RelayError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if !state.requested.is_empty() {
					return Ok(state.into_mut().and_then(|mut state| state.requested.pop_front()));
				}

				match state.modified() {
					Some(notify) => notify,
					None => return Ok(None),
				}
			};

			tokio::select! {
				// TODO make this fully async so we don't block requested()
				_ = self.refresh.tick() => self.register().await?,
				_ = notify => {},
			}
		}
	}

	pub async fn register(&mut self) -> Result<(), RelayError> {
		if let (Some(api), Some(node)) = (self.info.locals.api.as_ref(), self.info.locals.node.as_ref()) {
			// Refresh the origin in moq-api.
			let origin = moq_api::Origin { url: node.clone() };
			log::debug!("registering origin: namespace={} url={}", self.namespace, node);
			api.set_origin(&self.namespace, origin).await.map_err(Arc::new)?;
		}

		Ok(())
	}
}

impl ops::Deref for LocalProducer {
	type Target = Local;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

impl Drop for LocalProducer {
	fn drop(&mut self) {
		// TODO this is super lazy, but doing async stuff in Drop is annoying.
		let mut parent = self.parent.clone();
		let namespace = self.namespace.clone();
		tokio::spawn(async move { parent.unannounce(&namespace).await });
	}
}

/// Subscribe to a broadcast by requesting tracks.
///
/// This can be cloned to create handles.
#[derive(Clone)]
pub struct LocalConsumer {
	pub info: Arc<Local>,
	state: State<LocalState>,
}

impl LocalConsumer {
	fn new(info: Arc<Local>, state: State<LocalState>) -> Self {
		Self { info, state }
	}

	pub fn subscribe(&self, name: &str) -> Result<Option<LocalTrackReader>, RelayError> {
		let state = self.state.lock();

		// Try to reuse the track if there are still active readers
		if let Some(track) = state.tracks.get(name) {
			if let Some(track) = track.upgrade() {
				return Ok(Some(track));
			}
		}

		// Create a new track.
		let (writer, reader) = serve::Track {
			namespace: self.info.namespace.clone(),
			name: name.to_string(),
		}
		.produce();

		let reader = LocalTrackReader::new(reader, self.state.clone());

		// Upgrade the lock to mutable.
		let mut state = match state.into_mut() {
			Some(state) => state,
			None => return Ok(None),
		};

		// Insert the track into our Map so we deduplicate future requests.
		state.tracks.insert(name.to_string(), reader.downgrade());

		// Send the track to the writer to handle.
		state.requested.push_back(writer);

		Ok(Some(reader))
	}
}

impl ops::Deref for LocalConsumer {
	type Target = Local;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

#[derive(Clone)]
pub struct LocalTrackReader {
	pub reader: TrackReader,
	drop: Arc<LocalTrackDrop>,
}

impl LocalTrackReader {
	fn new(reader: TrackReader, parent: State<LocalState>) -> Self {
		let drop = Arc::new(LocalTrackDrop {
			parent,
			name: reader.name.clone(),
		});

		Self { reader, drop }
	}

	fn downgrade(&self) -> LocalTrackWeak {
		LocalTrackWeak {
			reader: self.reader.clone(),
			drop: Arc::downgrade(&self.drop),
		}
	}
}

impl ops::Deref for LocalTrackReader {
	type Target = TrackReader;

	fn deref(&self) -> &Self::Target {
		&self.reader
	}
}

impl ops::DerefMut for LocalTrackReader {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.reader
	}
}

struct LocalTrackWeak {
	reader: TrackReader,
	drop: Weak<LocalTrackDrop>,
}

impl LocalTrackWeak {
	fn upgrade(&self) -> Option<LocalTrackReader> {
		Some(LocalTrackReader {
			reader: self.reader.clone(),
			drop: self.drop.upgrade()?,
		})
	}
}

struct LocalTrackDrop {
	parent: State<LocalState>,
	name: String,
}

impl Drop for LocalTrackDrop {
	fn drop(&mut self) {
		if let Some(mut parent) = self.parent.lock_mut() {
			parent.tracks.remove(&self.name);
		}
	}
}
