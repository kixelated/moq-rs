use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use anyhow::Context;
use moq_transport::{model::broadcast, MoqError};

#[derive(Clone)]
pub struct Origin {
	// An API client used to get/set broadcasts.
	api: moq_api::Client,

	// The internal address of our node, prefixed with moq://
	node: http::Uri,

	// A map of active broadcasts.
	lookup: Arc<Mutex<HashMap<String, broadcast::Subscriber>>>,

	// A QUIC endpoint we'll use to fetch from other origins.
	quic: quinn::Endpoint,
}

impl Origin {
	pub fn new(api: moq_api::Client, node: http::Uri, quic: quinn::Endpoint) -> Self {
		Self {
			api,
			node,
			lookup: Default::default(),
			quic,
		}
	}

	pub async fn create_broadcast(&mut self, id: &str) -> Result<broadcast::Publisher, MoqError> {
		let (publisher, subscriber) = broadcast::new();

		// Check if a broadcast already exists by that id.
		match self.lookup.lock().unwrap().entry(id.to_string()) {
			hash_map::Entry::Occupied(_) => return Err(MoqError::Duplicate),
			hash_map::Entry::Vacant(v) => v.insert(subscriber),
		};

		let entry = moq_api::Broadcast {
			origin: self.node.clone(),
		};

		match self.api.set_broadcast(id, entry).await {
			Ok(_) => Ok(publisher),
			Err(err) => {
				self.lookup.lock().unwrap().remove(id);
				Err(MoqError::Unknown(err.to_string()))
			}
		}
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
						log::warn!("failed to run broadcast: id={} err={:?}", id, err);
					}
				}
				Err(err) => {
					log::warn!("failed to fetch broadcast: id={} err={:?}", id, err);
					publisher.close(MoqError::NotFound).ok();
				}
			}
		});

		subscriber
	}

	async fn fetch_broadcast(&mut self, id: &str) -> anyhow::Result<webtransport_quinn::Session> {
		let broadcast = match self.api.get_broadcast(id).await? {
			Some(broadcast) => broadcast,
			None => return Err(MoqError::NotFound.into()),
		};

		// Change the uri scheme to "https" for WebTransport
		// Also we need to add the broadcast id to the path.
		let mut parts = broadcast.origin.into_parts();
		parts.scheme = Some(http::uri::Scheme::HTTPS);
		parts.path_and_query = Some(format!("/{}", id).parse()?);
		let uri = http::Uri::from_parts(parts).context("failed to change scheme")?;

		log::debug!("connecting to origin: {}", uri);

		// Establish the webtransport session.
		let session = webtransport_quinn::connect(&self.quic, &uri)
			.await
			.context("failed to create WebTransport session")?;

		Ok(session)
	}

	async fn run_broadcast(
		&mut self,
		session: webtransport_quinn::Session,
		broadcast: broadcast::Publisher,
	) -> anyhow::Result<()> {
		let session = moq_transport::session::Client::subscriber(session, broadcast)
			.await
			.context("failed to establish MoQ session")?;

		session.run().await.context("failed to run MoQ session")?;

		Ok(())
	}

	pub async fn remove_broadcast(&mut self, id: &str) -> anyhow::Result<()> {
		self.lookup.lock().unwrap().remove(id).ok_or(MoqError::NotFound)?;
		self.api.delete_broadcast(id).await?;

		Ok(())
	}
}
