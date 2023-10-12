use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use moq_transport::cache::{broadcast, CacheError};
use url::Url;

use crate::RelayError;

#[derive(Clone)]
pub struct Origin {
	// An API client used to get/set broadcasts.
	// If None then we never use a remote origin.
	api: Option<moq_api::Client>,

	// The internal address of our node.
	// If None then we can never advertise ourselves as an origin.
	node: Option<Url>,

	// A map of active broadcasts.
	lookup: Arc<Mutex<HashMap<String, broadcast::Subscriber>>>,

	// A QUIC endpoint we'll use to fetch from other origins.
	quic: quinn::Endpoint,
}

impl Origin {
	pub fn new(api: Option<moq_api::Client>, node: Option<Url>, quic: quinn::Endpoint) -> Self {
		Self {
			api,
			node,
			lookup: Default::default(),
			quic,
		}
	}

	pub async fn create_broadcast(&mut self, id: &str) -> Result<broadcast::Publisher, RelayError> {
		let (publisher, subscriber) = broadcast::new();

		// Check if a broadcast already exists by that id.
		match self.lookup.lock().unwrap().entry(id.to_string()) {
			hash_map::Entry::Occupied(_) => return Err(CacheError::Duplicate.into()),
			hash_map::Entry::Vacant(v) => v.insert(subscriber),
		};

		if let Some(ref mut api) = self.api {
			// Make a URL for the broadcast.
			let url = self.node.as_ref().ok_or(RelayError::MissingNode)?.clone().join(id)?;

			log::info!("announcing origin: id={} url={}", id, url);

			let entry = moq_api::Origin { url };

			if let Err(err) = api.set_origin(id, entry).await {
				self.lookup.lock().unwrap().remove(id);
				return Err(err.into());
			}
		}

		Ok(publisher)
	}

	pub fn get_broadcast(&self, id: &str) -> broadcast::Subscriber {
		let mut lookup = self.lookup.lock().unwrap();

		if let Some(broadcast) = lookup.get(id) {
			if broadcast.closed().is_none() {
				return broadcast.clone();
			}
		}

		let (publisher, subscriber) = broadcast::new();
		lookup.insert(id.to_string(), subscriber.clone());

		let mut this = self.clone();
		let id = id.to_string();

		// Rather than fetching from the API and connecting via QUIC inline, we'll spawn a task to do it.
		// This way we could stop polling this session and it won't impact other session.
		// It also means we'll only connect the API and QUIC once if N subscribers suddenly show up.
		// However, the downside is that we don't return an error immediately.
		// If that's important, it can be done but it gets a bit racey.
		tokio::spawn(async move {
			match this.fetch_broadcast(&id).await {
				Ok(session) => {
					if let Err(err) = this.run_broadcast(session, publisher).await {
						log::warn!("failed to run broadcast: id={} err={:#?}", id, err);
					}
				}
				Err(err) => {
					log::warn!("failed to fetch broadcast: id={} err={:#?}", id, err);
					publisher.close(CacheError::NotFound).ok();
				}
			}
		});

		subscriber
	}

	async fn fetch_broadcast(&mut self, id: &str) -> Result<webtransport_quinn::Session, RelayError> {
		// Fetch the origin from the API.
		let api = match self.api {
			Some(ref mut api) => api,

			// We return NotFound here instead of earlier just to simulate an API fetch.
			None => return Err(CacheError::NotFound.into()),
		};

		log::info!("fetching origin: id={}", id);

		let origin = api.get_origin(id).await?.ok_or(CacheError::NotFound)?;

		log::info!("connecting to origin: url={}", origin.url);

		// Establish the webtransport session.
		let session = webtransport_quinn::connect(&self.quic, &origin.url).await?;

		Ok(session)
	}

	async fn run_broadcast(
		&mut self,
		session: webtransport_quinn::Session,
		broadcast: broadcast::Publisher,
	) -> Result<(), RelayError> {
		let session = moq_transport::session::Client::subscriber(session, broadcast).await?;

		session.run().await?;

		Ok(())
	}

	pub async fn remove_broadcast(&mut self, id: &str) -> Result<(), RelayError> {
		self.lookup.lock().unwrap().remove(id).ok_or(CacheError::NotFound)?;

		if let Some(ref mut api) = self.api {
			log::info!("deleting origin: id={}", id);
			api.delete_origin(id).await?;
		}

		Ok(())
	}
}
