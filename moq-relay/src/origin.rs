use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use anyhow::Context;
use moq_transport::{model::broadcast, MoqError};
use redis::{aio::ConnectionManager, AsyncCommands};

#[derive(Clone)]
pub struct Origin {
	// The internal address of our node, prefixed with moq://
	addr: http::Uri,

	// A map of active broadcasts.
	lookup: Arc<Mutex<HashMap<String, broadcast::Subscriber>>>,

	// A redis database to store active origins.
	redis: ConnectionManager,

	// A QUIC endpoint we'll use to fetch from other origins.
	quic: quinn::Endpoint,
}

impl Origin {
	pub fn new(addr: http::Uri, redis: ConnectionManager, quic: quinn::Endpoint) -> Self {
		Self {
			addr,
			lookup: Default::default(),
			redis,
			quic,
		}
	}

	pub async fn create_broadcast(&mut self, name: &str) -> Result<broadcast::Publisher, MoqError> {
		let (publisher, subscriber) = broadcast::new();

		// Check if a broadcast already exists by that name.
		match self.lookup.lock().unwrap().entry(name.to_string()) {
			hash_map::Entry::Occupied(_) => return Err(MoqError::Duplicate),
			hash_map::Entry::Vacant(v) => v.insert(subscriber),
		};

		let addr = self.addr.to_string();
		let key = Self::broadcast_key(name);

		log::debug!("inserting into redis: {} {}", key, addr);

		let res = redis::cmd("SET")
			.arg(&key)
			.arg(self.addr.to_string())
			.arg("NX")
			.arg("EX")
			.arg(60 * 60 * 24 * 7) // Set the key to expire in 7 days; just in case we forget to remove it.
			.query_async(&mut self.redis)
			.await;

		log::debug!("inserted: {:?}", res);

		// Store our origin address in redis.
		match res {
			// TODO we should create a separate error type for redis.
			Err(err) => Err(MoqError::Unknown(err.to_string())),

			// We successfully inserted our origin address, so return the broadcast.
			Ok(true) => Ok(publisher),

			// A broadcast already exists with the same name, so return an error.
			Ok(false) => {
				self.lookup.lock().unwrap().remove(name);
				Err(MoqError::Duplicate)
			}
		}
	}

	pub fn get_broadcast(&self, name: &str) -> broadcast::Subscriber {
		let (publisher, subscriber) = match self.lookup.lock().unwrap().entry(name.to_string()) {
			// We're already subscribed, so return the existing broadcast.
			hash_map::Entry::Occupied(entry) => return entry.get().clone(),

			// There's no existing broadcast, so we're going to create one.
			hash_map::Entry::Vacant(entry) => {
				let broadcast = broadcast::new();
				entry.insert(broadcast.1.clone());
				broadcast
			}
		};

		let mut this = self.clone();
		let name = name.to_string();

		// Rather than fetching from Redis and connecting via QUIC inline, we'll spawn a task to do it.
		// This way we could stop polling this session and it won't impact other session.
		// It also means we'll only connect to Redis and QUIC once if N subscribers suddenly show up.
		// However, the downside is that we don't return an error immediately.
		// If that's important, it can be done but it gets a bit racey.
		tokio::spawn(async move {
			match this.fetch_broadcast(&name).await {
				Ok(session) => {
					if let Err(err) = this.run_broadcast(session, publisher).await {
						log::warn!("failed to run broadcast: name={} err={:?}", name, err);
					}
				}
				Err(err) => {
					log::warn!("failed to fetch broadcast: name={} err={:?}", name, err);
					publisher.close(MoqError::NotFound).ok();
				}
			}
		});

		subscriber
	}

	async fn fetch_broadcast(&mut self, name: &str) -> anyhow::Result<webtransport_quinn::Session> {
		let key = Self::broadcast_key(name);

		log::debug!("getting from redis: {}", key);

		// Get the origin from redis.
		let uri: Option<String> = self.redis.get(&key).await?;

		let uri = match &uri {
			Some(uri) => http::Uri::try_from(uri)?,
			None => return Err(MoqError::NotFound.into()),
		};

		// Change the uri scheme to "https" for WebTransport
		// Also we need to add the broadcast name to the path.
		let mut parts = uri.into_parts();
		parts.scheme = Some(http::uri::Scheme::HTTPS);
		parts.path_and_query = Some(format!("/{}", name).parse()?);
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

	pub async fn remove_broadcast(&mut self, name: &str) -> Result<(), MoqError> {
		self.lookup.lock().unwrap().remove(name).ok_or(MoqError::NotFound)?;

		// TODO delete only if we're still the origin to be safe.
		let key = Self::broadcast_key(name);

		log::debug!("deleting from redis: {}", key);

		self.redis
			.del(key)
			.await
			.map_err(|e| MoqError::Unknown(e.to_string()))?;

		Ok(())
	}

	fn broadcast_key(name: &str) -> String {
		format!("broadcast.{}", name)
	}
}
