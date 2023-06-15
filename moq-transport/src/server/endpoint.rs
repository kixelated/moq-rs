use super::handshake::{Accept, Connecting};

use anyhow::Context;
use tokio::task::JoinSet;

pub struct Endpoint {
	// The QUIC server, yielding new connections and sessions.
	endpoint: quinn::Endpoint,

	// A list of connections that are completing the WebTransport handshake.
	handshake: JoinSet<anyhow::Result<Accept>>,
}

impl Endpoint {
	pub fn new(endpoint: quinn::Endpoint) -> Self {
		let handshake = JoinSet::new();
		Self { endpoint, handshake }
	}

	// Accept the next WebTransport session.
	pub async fn accept(&mut self) -> anyhow::Result<Accept> {
		loop {
			tokio::select!(
				// Accept the connection and start the WebTransport handshake.
				conn = self.endpoint.accept() => {
					let conn = conn.context("failed to accept connection")?;
					self.handshake.spawn(async move {
						Connecting::new(conn).accept().await
					});
				},
				// Return any mostly finished WebTransport handshakes.
				res = self.handshake.join_next(), if !self.handshake.is_empty() => {
					let res = res.expect("no tasks").expect("task aborted");
					match res {
						Ok(session) => return Ok(session),
						Err(err) => log::warn!("failed to accept session: {:?}", err),
					}
				},
			)
		}
	}
}
