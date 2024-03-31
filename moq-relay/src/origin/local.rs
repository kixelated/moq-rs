use std::collections::hash_map;
use std::collections::HashMap;

use std::collections::VecDeque;
use std::fmt;
use std::ops;
use std::sync::Arc;
use std::sync::Weak;

use moq_transport::serve::{self, ServeError, TrackReader, TrackWriter};
use moq_transport::util::State;
use url::Url;

use crate::RelayError;
pub struct Locals {
	pub api: Option<moq_api::Client>,
	pub node: Option<Url>,
}

impl Locals {
	pub fn produce(self) -> (LocalsProducer, LocalsConsumer) {
		let (send, recv) = State::default();
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

	pub fn announce(&mut self, namespace: String) -> Result<LocalProducer, RelayError> {
		let mut state = self.state.lock_mut().ok_or(ServeError::Done)?;

		let entry = match state.lookup.entry(namespace.clone()) {
			hash_map::Entry::Vacant(entry) => entry,
			hash_map::Entry::Occupied(_) => return Err(ServeError::Duplicate.into()),
		};

		let (writer, reader) = Local { namespace }.produce(self.clone());

		entry.insert(reader);

		Ok(writer)
	}

	pub fn unannounce(&mut self, namespace: &str) -> Result<(), RelayError> {
		let mut state = self.state.lock_mut().ok_or(ServeError::Done)?;
		state.lookup.remove(namespace).ok_or(ServeError::NotFound)?;

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
	info: Arc<Locals>,
	state: State<LocalsState>,
}

impl LocalsConsumer {
	fn new(info: Arc<Locals>, state: State<LocalsState>) -> Self {
		Self { info, state }
	}

	pub fn find(&self, namespace: &str) -> Option<LocalConsumer> {
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
}

impl Local {
	/// Create a new broadcast.
	fn produce(self, parent: LocalsProducer) -> (LocalProducer, LocalConsumer) {
		let (send, recv) = State::default();
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

	refresh: tokio::time::Interval,
	parent: LocalsProducer,
}

impl LocalProducer {
	fn new(info: Arc<Local>, state: State<LocalState>, parent: LocalsProducer) -> Self {
		let refresh = tokio::time::interval(tokio::time::Duration::from_secs(300));

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
					let mut state = state.into_mut().ok_or(ServeError::Done)?;
					return Ok(state.requested.pop_front());
				}

				match state.modified() {
					Some(notify) => notify,
					None => return Ok(None),
				}
			};

			tokio::select! {
				// TODO make this fully async so we don't block requested()
				res = self.refresh() => res?,
				_ = notify => {},
			}
		}
	}

	async fn refresh(&mut self) -> Result<(), RelayError> {
		self.refresh.tick().await;

		if let (Some(api), Some(node)) = (self.parent.api.as_ref(), self.parent.node.as_ref()) {
			// Refresh the origin in moq-api.
			let origin = moq_api::Origin { url: node.clone() };
			api.set_origin(&self.info.namespace, origin).await.map_err(Arc::new)?;
		}

		Ok(())
	}
}

impl Drop for LocalProducer {
	fn drop(&mut self) {
		let namespace = self.namespace.to_string();
		self.parent.unannounce(&namespace).ok();
	}
}

impl ops::Deref for LocalProducer {
	type Target = Local;

	fn deref(&self) -> &Self::Target {
		&self.info
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

	pub fn subscribe(&self, name: &str) -> Result<LocalTrackReader, RelayError> {
		let state = self.state.lock();

		// Try to reuse the track if there are still active readers
		if let Some(track) = state.tracks.get(name) {
			if let Some(track) = track.upgrade() {
				return Ok(track);
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
		let mut state = state.into_mut().ok_or(ServeError::Done)?;

		// Insert the track into our Map so we deduplicate future requests.
		state.tracks.insert(name.to_string(), reader.downgrade());

		// Send the track to the writer to handle.
		state.requested.push_back(writer);

		Ok(reader)
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
