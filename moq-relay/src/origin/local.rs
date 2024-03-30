use std::collections::hash_map;
use std::collections::HashMap;

use std::collections::VecDeque;
use std::ops;
use std::sync::Arc;

use moq_transport::serve::TrackReaderWeak;
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

		let (publisher, subscriber) = Local {
			locals: self.info.clone(),
			namespace,
		}
		.produce();

		entry.insert(subscriber);

		Ok(publisher)
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
	pub locals: Arc<Locals>,
	pub namespace: String,
}

impl Local {
	/// Create a new broadcast.
	pub fn produce(self) -> (LocalProducer, LocalConsumer) {
		let (send, recv) = State::default();
		let info = Arc::new(self);

		let publisher = LocalProducer::new(info.clone(), send);
		let subscriber = LocalConsumer::new(info, recv);

		(publisher, subscriber)
	}
}

impl ops::Deref for Local {
	type Target = Locals;

	fn deref(&self) -> &Self::Target {
		&self.locals
	}
}

#[derive(Default)]
struct LocalState {
	tracks: HashMap<String, TrackReaderWeak>,
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
}

impl LocalProducer {
	fn new(info: Arc<Local>, state: State<LocalState>) -> Self {
		let refresh = tokio::time::interval(tokio::time::Duration::from_secs(300));

		Self { info, state, refresh }
	}

	/// Block until the next track requested by a subscriber.
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

		if let (Some(api), Some(node)) = (self.info.api.as_ref(), self.info.node.as_ref()) {
			// Refresh the origin in moq-api.
			let origin = moq_api::Origin { url: node.clone() };
			api.set_origin(&self.info.namespace, origin).await.map_err(Arc::new)?;
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

	pub fn subscribe(&self, name: &str) -> Result<TrackReader, RelayError> {
		let state = self.state.lock();

		// Insert the track into our Map so we deduplicate future requests.
		if let Some(track) = state.tracks.get(name) {
			if let Some(track) = track.upgrade() {
				return Ok(track.clone());
			}
		}

		// Create a new track.
		let (publisher, subscriber) = serve::Track {
			namespace: self.info.namespace.clone(),
			name: name.to_string(),
		}
		.produce();

		// Upgrade the lock to mutable.
		let mut state = state.into_mut().ok_or(ServeError::Done)?;

		// Insert the track into our Map so we deduplicate future requests.
		state.tracks.insert(name.to_string(), subscriber.weak());

		// Send the track to the Publisher to handle.
		state.requested.push_back(publisher);

		Ok(subscriber)
	}
}

impl ops::Deref for LocalConsumer {
	type Target = Local;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
