use super::{Control, Publisher, SessionError, Subscriber};
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

		log::debug!("received client SETUP: {:?}", client);

		if !client.versions.contains(&setup::Version::DRAFT_03) {
			return Err(SessionError::Version(
				client.versions,
				[setup::Version::DRAFT_03].into(),
			));
		}

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
		let setup = self.setup(setup::Role::Publisher)?;
		setup.encode(&mut self.control.0).await?;

		let control = Control::new(self.control.0, self.control.1);
		let publisher = Publisher::new(self.session, control, source);
		Ok(publisher)
	}

	/// Accept the session as a subscriber only.
	pub async fn subscriber(mut self, source: broadcast::Publisher) -> Result<Subscriber, SessionError> {
		let setup = self.setup(setup::Role::Subscriber)?;
		setup.encode(&mut self.control.0).await?;

		let control = Control::new(self.control.0, self.control.1);
		let subscriber = Subscriber::new(self.session, control, source);
		Ok(subscriber)
	}

	// TODO Accept the session and perform both roles.
	/*
	pub async fn accept(self) -> anyhow::Result<(Publisher, Subscriber)> {
		self.ok(setup::Role::Both).await
	}
	*/

	fn setup(&mut self, role: setup::Role) -> Result<setup::Server, SessionError> {
		let server = setup::Server {
			role,
			version: setup::Version::DRAFT_03,
			params: Default::default(),
		};

		log::debug!("sending server SETUP: {:?}", server);

		// We need to sure we support the opposite of the client's role.
		// ex. if the client is a publisher, we must be a subscriber ONLY.
		if !self.client.role.is_compatible(server.role) {
			return Err(SessionError::RoleIncompatible(self.client.role, server.role));
		}

		Ok(server)
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
