use super::{OrClose, Stream};

use crate::{
	message,
	setup::{self, Extensions},
	Error, Publisher, Session, Subscriber,
};

pub struct Server {
	session: Session,
}

impl Server {
	pub fn new(session: web_transport::Session) -> Self {
		Self {
			session: Session::new(session),
		}
	}

	pub async fn accept_publisher(self) -> Result<Publisher, Error> {
		let (publisher, _) = self.accept_role(setup::Role::Publisher).await?;
		Ok(publisher.unwrap())
	}

	pub async fn accept_subscriber(self) -> Result<Subscriber, Error> {
		let (_, subscriber) = self.accept_role(setup::Role::Subscriber).await?;
		Ok(subscriber.unwrap())
	}

	/// Accept a session as both a publisher and subscriber.
	pub async fn accept(self) -> Result<(Publisher, Subscriber), Error> {
		self.accept_role(setup::Role::Both)
			.await
			.map(|(publisher, subscriber)| (publisher.unwrap(), subscriber.unwrap()))
	}

	/// Accept a session as either a publisher, subscriber, or both, as chosen by the client.
	pub async fn accept_any(self) -> Result<(Option<Publisher>, Option<Subscriber>), Error> {
		self.accept_role(setup::Role::Any).await
	}

	pub async fn accept_role(mut self, role: setup::Role) -> Result<(Option<Publisher>, Option<Subscriber>), Error> {
		let mut stream = self.session.accept().await?;
		let kind = stream.reader.decode().await?;

		if kind != message::Stream::Session {
			return Err(Error::UnexpectedStream(kind));
		}

		let role = Self::setup(&mut stream, role).await.or_close(&mut stream)?;

		Ok(Session::start(self.session, role, stream))
	}

	async fn setup(control: &mut Stream, server_role: setup::Role) -> Result<setup::Role, Error> {
		let client: setup::Client = control.reader.decode().await?;

		if !client.versions.contains(&setup::Version::FORK_00) {
			return Err(Error::Version(client.versions, [setup::Version::FORK_00].into()));
		}

		let client_role = client.extensions.get()?.unwrap_or_default();

		let role = server_role
			.downgrade(client_role)
			.ok_or(Error::RoleIncompatible(client_role, server_role))?;

		let mut extensions = Extensions::default();
		extensions.set(role);

		let server = setup::Server {
			version: setup::Version::FORK_00,
			extensions,
		};

		control.writer.encode(&server).await?;

		tracing::info!(version = ?server.version, ?role, "connected");

		Ok(server_role)
	}
}
