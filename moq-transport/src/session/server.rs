use super::{Publisher, Subscriber};
use crate::{model::broadcast, setup};

use webtransport_quinn::{RecvStream, SendStream, Session};

use anyhow::Context;

/// An endpoint that accepts connections, publishing and/or consuming live streams.
pub struct Server {}

impl Server {
	/// Accept an established Webtransport session, performing the MoQ handshake.
	///
	/// This returns a [Request] half-way through the handshake that allows the application to accept or deny the session.
	pub async fn accept(session: Session) -> anyhow::Result<Request> {
		let mut control = session.accept_bi().await.context("failed to accept bidi stream")?;

		let client = setup::Client::decode(&mut control.1)
			.await
			.context("failed to read CLIENT SETUP")?;

		client
			.versions
			.iter()
			.find(|version| **version == setup::Version::KIXEL_00)
			.context("no supported versions")?;

		Ok(Request {
			session,
			client,
			control,
		})
	}
}

/// A partially complete MoQ Transport handshake.
pub struct Request {
	session: Session,
	client: setup::Client,
	control: (SendStream, RecvStream),
}

impl Request {
	/// Accept the session as a publisher, using the provided broadcast to serve subscriptions.
	pub async fn publisher(mut self, source: broadcast::Subscriber) -> anyhow::Result<Publisher> {
		self.send_setup(setup::Role::Publisher).await?;

		let publisher = Publisher::new(self.session, self.control, source);
		Ok(publisher)
	}

	/// Accept the session as a subscriber only.
	pub async fn subscriber(mut self, source: broadcast::Publisher) -> anyhow::Result<Subscriber> {
		self.send_setup(setup::Role::Subscriber).await?;

		let subscriber = Subscriber::new(self.session, self.control, source);
		Ok(subscriber)
	}

	// TODO Accept the session and perform both roles.
	/*
	pub async fn accept(self) -> anyhow::Result<(Publisher, Subscriber)> {
		self.ok(setup::Role::Both).await
	}
	*/

	async fn send_setup(&mut self, role: setup::Role) -> anyhow::Result<()> {
		let server = setup::Server {
			role,
			version: setup::Version::KIXEL_00,
		};

		// We need to sure we support the opposite of the client's role.
		// ex. if the client is a publisher, we must be a subscriber ONLY.
		if !self.client.role.is_compatible(server.role) {
			anyhow::bail!(
				"incompatible roles: client={:?} server={:?}",
				self.client.role,
				server.role
			);
		}

		server
			.encode(&mut self.control.0)
			.await
			.context("failed to send setup server")?;

		Ok(())
	}

	/// Reject the request, closing the Webtransport session.
	pub fn reject(self, code: u32) {
		self.session.close(code, b"")
	}

	/// The role advertised by the client.
	pub fn role(&self) -> setup::Role {
		self.client.role
	}
}
