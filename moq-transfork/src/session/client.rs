use super::{OrClose, Stream};
use crate::{message, setup, Publisher, Session, SessionError, Subscriber};

pub struct Client {
	session: Session,
}

impl Client {
	pub fn new(session: web_transport::Session) -> Self {
		Self {
			session: Session::new(session),
		}
	}

	/// Connect a session as both a publisher and subscriber.
	pub async fn both(self) -> Result<(Publisher, Subscriber), SessionError> {
		self.role(setup::Role::Both)
			.await
			.map(|(publisher, subscriber)| (publisher.unwrap(), subscriber.unwrap()))
	}

	/// Connect a session as either a publisher, subscriber, or both, as chosen by server.
	pub async fn any(self) -> Result<(Option<Publisher>, Option<Subscriber>), SessionError> {
		self.role(setup::Role::Any).await
	}

	pub async fn role(mut self, role: setup::Role) -> Result<(Option<Publisher>, Option<Subscriber>), SessionError> {
		let mut stream = self.session.open(message::Stream::Session).await?;

		let role = Self::setup(&mut stream, role).await.or_close(&mut stream)?;
		Ok(Session::start(self.session, role, stream))
	}

	async fn setup(setup: &mut Stream, client_role: setup::Role) -> Result<setup::Role, SessionError> {
		let mut extensions = setup::Extensions::default();
		extensions.set(client_role)?;

		let client = setup::Client {
			versions: [setup::Version::FORK_00].into(),
			extensions,
		};

		setup.writer.encode(&client).await?;

		let server: setup::Server = setup.reader.decode().await?;
		let server_role = server.extensions.get()?.unwrap_or_default();

		let role = client_role
			.downgrade(server_role)
			.ok_or(SessionError::RoleIncompatible(client_role, server_role))?;

		if client_role != role {
			tracing::debug!(?role, "client downgraded");
		}

		tracing::info!(version = ?server.version, role = ?role, "connected");

		Ok(role)
	}

	pub async fn publisher(self) -> Result<Publisher, SessionError> {
		let (publisher, _) = self.role(setup::Role::Publisher).await?;
		Ok(publisher.unwrap())
	}

	pub async fn subscriber(self) -> Result<Subscriber, SessionError> {
		let (_, subscriber) = self.role(setup::Role::Subscriber).await?;
		Ok(subscriber.unwrap())
	}
}
