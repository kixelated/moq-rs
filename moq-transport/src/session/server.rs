use super::{Publisher, SessionError, Subscriber};
use crate::{cache::broadcast, setup};

use webtransport_quinn::{RecvStream, SendStream, Session};

/// An endpoint that accepts connections, publishing and/or consuming live streams.
pub struct Server {}

impl Server {
	/// Accept an established Webtransport session, performing the MoQ handshake.
	///
	/// This returns a [Request] half-way through the handshake that allows the application to accept or deny the session.
	pub async fn accept(session: Session) -> Result<Request, SessionError> {
		let mut control = session.accept_bi().await?;

		let client = setup::Client::decode(&mut control.1).await?;

		client
			.versions
			.iter()
			.find(|version| **version == setup::Version::KIXEL_01)
			.ok_or_else(|| SessionError::Version(client.versions.clone(), vec![setup::Version::KIXEL_01].into()))?;

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
	pub async fn publisher(mut self, source: broadcast::Subscriber) -> Result<Publisher, SessionError> {
		self.send_setup(setup::Role::Publisher).await?;

		let publisher = Publisher::new(self.session, self.control, source);
		Ok(publisher)
	}

	/// Accept the session as a subscriber only.
	pub async fn subscriber(mut self, source: broadcast::Publisher) -> Result<Subscriber, SessionError> {
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

	async fn send_setup(&mut self, role: setup::Role) -> Result<(), SessionError> {
		let server = setup::Server {
			role,
			version: setup::Version::KIXEL_01,
			params: Default::default(),
		};

		// We need to sure we support the opposite of the client's role.
		// ex. if the client is a publisher, we must be a subscriber ONLY.
		if !self.client.role.is_compatible(server.role) {
			return Err(SessionError::RoleIncompatible(self.client.role, server.role));
		}

		server.encode(&mut self.control.0).await?;

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
