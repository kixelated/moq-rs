use std::collections::hash_map;
use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};

use std::collections::VecDeque;

use moq_transport::serve::{self, ServeError, TrackPublisher, TrackSubscriber};
use moq_transport::session::SessionError;
use moq_transport::util::Watch;
use url::Url;

#[derive(Clone)]
pub struct Origin {
	// An API client used to get/set broadcasts.
	// If None then we never use a remote origin.
	// TODO: Stub this out instead.
	_api: Option<moq_api::Client>,

	// The internal address of our node.
	// If None then we can never advertise ourselves as an origin.
	// TODO: Stub this out instead.
	_node: Option<Url>,

	// A map of active broadcasts by namespace.
	origins: Arc<Mutex<HashMap<String, OriginSubscriber>>>,

	// A QUIC endpoint we'll use to fetch from other origins.
	_quic: quinn::Endpoint,
}

impl Origin {
	pub fn new(_api: Option<moq_api::Client>, _node: Option<Url>, _quic: quinn::Endpoint) -> Self {
		Self {
			_api,
			_node,
			origins: Default::default(),
			_quic,
		}
	}

	pub fn announce(&self, namespace: &str) -> Result<OriginPublisher, SessionError> {
		let mut origins = self.origins.lock().unwrap();
		let entry = match origins.entry(namespace.to_string()) {
			hash_map::Entry::Vacant(entry) => entry,
			hash_map::Entry::Occupied(_) => return Err(ServeError::Duplicate.into()),
		};

		let (publisher, subscriber) = self.produce(namespace);
		entry.insert(subscriber);

		Ok(publisher)
	}

	/*
	// Create a publisher that constantly updates itself as the origin in moq-api.
	// It holds a reference to the subscriber to prevent dropping early.
	let mut publisher = Publisher {
		broadcast: publisher,
		subscriber,
		api: None,
	};

	// Insert the publisher into the database.
	if let Some(api) = self.api.as_mut() {
		// Make a URL for the broadcast.
		let url = self.node.as_ref().ok_or(RelayError::MissingNode)?.clone().join(id)?;
		let origin = moq_api::Origin { url };
		api.set_origin(id, &origin).await?;

		// Refresh every 5 minutes
		publisher.api = Some((api.clone(), origin));
	}


	Ok(())
	*/

	pub fn subscribe(&self, namespace: &str, name: &str) -> Result<serve::TrackSubscriber, SessionError> {
		let mut origin = self
			.origins
			.lock()
			.unwrap()
			.get(namespace)
			.cloned()
			.ok_or(ServeError::NotFound)?;

		let track = origin.request_track(name)?;
		Ok(track)
		/*
		let mut routes = self.local.lock().unwrap();

		if let Some(broadcast) = routes.get(id) {
			if let Some(broadcast) = broadcast.upgrade() {
				return broadcast;
			}
		}

		let (publisher, subscriber) = broadcast::new(id);
		let subscriber = Arc::new(Subscriber {
			broadcast: subscriber,
			origin: self.clone(),
		});

		cache.insert(id.to_string(), Arc::downgrade(&subscriber));

		let mut this = self.clone();
		let id = id.to_string();

		// Rather than fetching from the API and connecting via QUIC inline, we'll spawn a task to do it.
		// This way we could stop polling this session and it won't impact other session.
		// It also means we'll only connect the API and QUIC once if N subscribers suddenly show up.
		// However, the downside is that we don't return an error immediately.
		// If that's important, it can be done but it gets a bit racey.
		tokio::spawn(async move {
			if let Err(err) = this.serve(&id, publisher).await {
				log::warn!("failed to serve remote broadcast: id={} err={}", id, err);
			}
		});

		subscriber
		*/
	}

	/*
	async fn serve(&mut self, id: &str, publisher: broadcast::Publisher) -> Result<(), RelayError> {
		log::debug!("finding origin: id={}", id);

		// Fetch the origin from the API.
		let origin = self
			.api
			.as_mut()
			.ok_or(ServeError::NotFound)?
			.get_origin(id)
			.await?
			.ok_or(ServeError::NotFound)?;

		log::debug!("fetching from origin: id={} url={}", id, origin.url);

		// Establish the webtransport session.
		let session = webtransport_quinn::connect(&self.quic, &origin.url).await?;
		let session = moq_transport::session::Client::subscriber(session, publisher).await?;

		session.run().await?;

		Ok(())
	}
	*/

	/// Create a new broadcast.
	fn produce(&self, namespace: &str) -> (OriginPublisher, OriginSubscriber) {
		let state = Watch::new(State::new(namespace));

		let publisher = OriginPublisher::new(state.clone());
		let subscriber = OriginSubscriber::new(state);

		(publisher, subscriber)
	}
}

#[derive(Debug)]
struct State {
	namespace: String,
	tracks: HashMap<String, TrackSubscriber>,
	requested: VecDeque<TrackPublisher>,
	closed: Result<(), ServeError>,
}

impl State {
	pub fn new(namespace: &str) -> Self {
		Self {
			namespace: namespace.to_string(),
			tracks: HashMap::new(),
			requested: VecDeque::new(),
			closed: Ok(()),
		}
	}

	pub fn get_track(&self, name: &str) -> Result<Option<TrackSubscriber>, ServeError> {
		// Insert the track into our Map so we deduplicate future requests.
		if let Some(track) = self.tracks.get(name) {
			return Ok(Some(track.clone()));
		}

		self.closed.clone()?;
		Ok(None)
	}

	pub fn request_track(&mut self, name: &str) -> Result<TrackSubscriber, ServeError> {
		// Insert the track into our Map so we deduplicate future requests.
		let entry = match self.tracks.entry(name.to_string()) {
			hash_map::Entry::Vacant(entry) => entry,
			hash_map::Entry::Occupied(entry) => return Ok(entry.get().clone()),
		};

		self.closed.clone()?;

		// Create a new track.
		let (publisher, subscriber) = serve::Track {
			namespace: self.namespace.clone(),
			name: name.to_string(),
		}
		.produce();

		// Deduplicate with others
		// TODO This should be weak
		entry.insert(subscriber.clone());

		// Send the track to the Publisher to handle.
		self.requested.push_back(publisher);

		Ok(subscriber)
	}

	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}
}

impl Drop for State {
	fn drop(&mut self) {
		for mut track in self.requested.drain(..) {
			track.close(ServeError::NotFound).ok();
		}
	}
}

/// Publish new tracks for a broadcast by name.
pub struct OriginPublisher {
	state: Watch<State>,
}

impl OriginPublisher {
	fn new(state: Watch<State>) -> Self {
		Self { state }
	}

	/// Block until the next track requested by a subscriber.
	pub async fn requested(&mut self) -> Result<serve::TrackPublisher, ServeError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if !state.requested.is_empty() {
					return Ok(state.into_mut().requested.pop_front().unwrap());
				}

				state.closed.clone()?;
				state.changed()
			};

			notify.await;
		}
	}
}

impl Drop for OriginPublisher {
	fn drop(&mut self) {
		self.state.lock_mut().close(ServeError::Done).ok();
	}
}

/// Subscribe to a broadcast by requesting tracks.
///
/// This can be cloned to create handles.
#[derive(Clone)]
pub struct OriginSubscriber {
	state: Watch<State>,
	_dropped: Arc<Dropped>,
}

impl OriginSubscriber {
	fn new(state: Watch<State>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self { state, _dropped }
	}

	pub fn get_track(&self, name: &str) -> Result<Option<TrackSubscriber>, ServeError> {
		self.state.lock_mut().get_track(name)
	}

	pub fn request_track(&mut self, name: &str) -> Result<TrackSubscriber, ServeError> {
		self.state.lock_mut().request_track(name)
	}

	/// Wait until if the broadcast is closed, either because the publisher was dropped or called [Publisher::close].
	pub async fn closed(&self) -> ServeError {
		loop {
			let notify = {
				let state = self.state.lock();
				if let Some(err) = state.closed.as_ref().err() {
					return err.clone();
				}

				state.changed()
			};

			notify.await;
		}
	}
}

struct Dropped {
	state: Watch<State>,
}

impl Dropped {
	fn new(state: Watch<State>) -> Self {
		Self { state }
	}
}

impl Drop for Dropped {
	fn drop(&mut self) {
		self.state.lock_mut().close(ServeError::Done).ok();
	}
}

/*
pub struct Subscriber {
	pub broadcast: broadcast::Subscriber,

	origin: Origin,
}

impl Drop for Subscriber {
	fn drop(&mut self) {
		self.origin.cache.lock().unwrap().remove(&self.broadcast.id);
	}
}

impl Deref for Subscriber {
	type Target = broadcast::Subscriber;

	fn deref(&self) -> &Self::Target {
		&self.broadcast
	}
}

pub struct Publisher {
	pub broadcast: broadcast::Publisher,

	api: Option<(moq_api::Client, moq_api::Origin)>,

	#[allow(dead_code)]
	subscriber: Arc<Subscriber>,
}

impl Publisher {
	pub async fn run(&mut self) -> Result<(), ApiError> {
		// Every 5m tell the API we're still alive.
		// TODO don't hard-code these values
		let mut interval = time::interval(time::Duration::from_secs(60 * 5));

		loop {
			if let Some((api, origin)) = self.api.as_mut() {
				api.patch_origin(&self.broadcast.id, origin).await?;
			}

			// TODO move to start of loop; this is just for testing
			interval.tick().await;
		}
	}

	pub async fn close(&mut self) -> Result<(), ApiError> {
		if let Some((api, _)) = self.api.as_mut() {
			api.delete_origin(&self.broadcast.id).await?;
		}

		Ok(())
	}
}

impl Deref for Publisher {
	type Target = broadcast::Publisher;

	fn deref(&self) -> &Self::Target {
		&self.broadcast
	}
}

impl DerefMut for Publisher {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.broadcast
	}
}

*/
