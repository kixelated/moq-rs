use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use anyhow::Context;

use moq_transport::{model::broadcast, session::Request, setup::Role};

#[derive(Clone)]
pub struct Session {
	broadcasts: Arc<Mutex<HashMap<String, broadcast::Subscriber>>>,
}

impl Session {
	pub fn new(broadcasts: Arc<Mutex<HashMap<String, broadcast::Subscriber>>>) -> Self {
		Self { broadcasts }
	}

	pub async fn run(&mut self, conn: quinn::Connecting) -> anyhow::Result<()> {
		// Wait for the QUIC connection to be established.
		let conn = conn.await.context("failed to establish QUIC connection")?;

		// Wait for the CONNECT request.
		let request = webtransport_quinn::accept(conn)
			.await
			.context("failed to receive WebTransport request")?;

		let path = request.uri().path().to_string();

		// Accept the CONNECT request.
		let session = request
			.ok()
			.await
			.context("failed to respond to WebTransport request")?;

		// Perform the MoQ handshake.
		let request = moq_transport::session::Server::accept(session)
			.await
			.context("failed to accept handshake")?;

		let role = request.role();

		match role {
			Role::Publisher => {
				log::info!("publisher start: path={}", path);

				if let Err(err) = self.serve_publisher(request, &path).await {
					log::warn!("publisher error: path={} err={}", path, err);
				}
			}
			Role::Subscriber => {
				log::info!("subscriber start: path={}", path);

				if let Err(err) = self.serve_subscriber(request, &path).await {
					log::warn!("subscriber error: path={} err={}", path, err)
				}
			}
			Role::Both => request.reject(300),
		};

		Ok(())
	}

	async fn serve_publisher(&mut self, request: Request, path: &str) -> anyhow::Result<()> {
		let (publisher, subscriber) = broadcast::new(path);

		match self.broadcasts.lock().unwrap().entry(path.to_string()) {
			hash_map::Entry::Occupied(_) => {
				request.reject(409);
				return Ok(());
			}
			hash_map::Entry::Vacant(entry) => entry.insert(subscriber),
		};

		let session = request.subscriber(publisher).await?;
		session.run().await?;

		Ok(())
	}

	async fn serve_subscriber(&mut self, request: Request, path: &str) -> anyhow::Result<()> {
		let broadcast = self.broadcasts.lock().unwrap().get(path).cloned();

		if let Some(broadcast) = broadcast {
			let session = request.publisher(broadcast.clone()).await?;
			session.run().await?;
		} else {
			request.reject(404);
		};

		Ok(())
	}
}
