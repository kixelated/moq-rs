use crate::{app, media, transport};

use anyhow::Context;

pub struct Server {
	// The QUIC server, yielding new connections and sessions.
	transport: transport::Server,

	// The media source
	broadcast: media::source::Broadcast,
}

impl Server {
	// Create a new server
	pub fn new(transport: transport::Server, broadcast: media::source::Broadcast) -> Self {
		Self { transport, broadcast }
	}

	pub async fn run(&mut self) -> anyhow::Result<()> {
		loop {
			let conn = self.transport.accept().await.context("failed to accept connection")?;
			let broadcast = self.broadcast.subscribe();

			tokio::spawn(async move {
				if let Err(e) = Self::run_conn(conn, broadcast).await {
					log::error!("connection closed: {:?}", e);
				}
			});
		}
	}

	async fn run_conn(conn: transport::Connection, broadcast: media::broadcast::Subscriber) -> anyhow::Result<()> {
		let session = conn.connect().await.context("failed to accept session")?;
		let session = app::Session::new(session);

		session.serve_broadcast(broadcast).await
	}
}
