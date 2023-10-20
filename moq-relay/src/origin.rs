use std::ops::{Deref, DerefMut};
use std::{
	collections::HashMap,
	sync::{Arc, Mutex, Weak},
};

use moq_api::ApiError;
use moq_transport::cache::{broadcast, CacheError};
use url::Url;

use tokio::time;

use crate::RelayError;

#[derive(Clone)]
pub struct Origin {
	// An API client used to get/set broadcasts.
	// If None then we never use a remote origin.
	// TODO: Stub this out instead.
	api: Option<moq_api::Client>,

	// The internal address of our node.
	// If None then we can never advertise ourselves as an origin.
	// TODO: Stub this out instead.
	node: Option<Url>,

	// A map of active broadcasts by ID.
	cache: Arc<Mutex<HashMap<String, Weak<Subscriber>>>>,

	// A QUIC endpoint we'll use to fetch from other origins.
	quic: quinn::Endpoint,
}

impl Origin {
	pub fn new(api: Option<moq_api::Client>, node: Option<Url>, quic: quinn::Endpoint) -> Self {
		Self {
			api,
			node,
			cache: Default::default(),
			quic,
		}
	}

	/// Create a new broadcast with the given ID.
	///
	/// Publisher::run needs to be called to periodically refresh the origin cache.
	pub async fn publish(&mut self, id: &str) -> Result<Publisher, RelayError> {
		let (publisher, subscriber) = broadcast::new(id);

		let subscriber = {
			let mut cache = self.cache.lock().unwrap();

			// Check if the broadcast already exists.
			// TODO This is racey, because a new publisher could be created while existing subscribers are still active.
			if cache.contains_key(id) {
				return Err(CacheError::Duplicate.into());
			}

			// Create subscriber that will remove from the cache when dropped.
			let subscriber = Arc::new(Subscriber {
				broadcast: subscriber,
				origin: self.clone(),
			});

			cache.insert(id.to_string(), Arc::downgrade(&subscriber));

			subscriber
		};

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

		Ok(publisher)
	}

	pub fn subscribe(&self, id: &str) -> Arc<Subscriber> {
		let mut cache = self.cache.lock().unwrap();

		if let Some(broadcast) = cache.get(id) {
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
	}

	async fn serve(&mut self, id: &str, publisher: broadcast::Publisher) -> Result<(), RelayError> {
		log::debug!("finding origin: id={}", id);

		// Fetch the origin from the API.
		let origin = self
			.api
			.as_mut()
			.ok_or(CacheError::NotFound)?
			.get_origin(id)
			.await?
			.ok_or(CacheError::NotFound)?;

		log::debug!("fetching from origin: id={} url={}", id, origin.url);

		// Establish the webtransport session.
		let session = webtransport_quinn::connect(&self.quic, &origin.url).await?;
		let session = moq_transport::session::Client::subscriber(session, publisher).await?;

		session.run().await?;

		Ok(())
	}
}

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
