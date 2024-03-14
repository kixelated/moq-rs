use crate::{
	control::{self, MessageSource},
	data,
	error::SessionError,
	publisher, setup, subscriber,
};

pub struct Session {
	webtransport: webtransport_quinn::Session,
	publisher: Option<publisher::Session>,
	subscriber: Option<subscriber::Session>,
	control: (webtransport_quinn::SendStream, webtransport_quinn::RecvStream),
}

impl Session {
	pub async fn accept(session: webtransport_quinn::Session, role: setup::Role) -> Result<Self, SessionError> {
		let mut control = session.accept_bi().await?;

		let client = setup::Client::decode(&mut control.1).await?;

		log::debug!("received client SETUP: {:?}", client);

		if !client.versions.contains(&setup::Version::DRAFT_03) {
			return Err(SessionError::Version(
				client.versions,
				[setup::Version::DRAFT_03].into(),
			));
		}

		// Downgrade our role based on the client's role.
		let role = match client.role {
			setup::Role::Both => role,
			setup::Role::Publisher => match role {
				// Both sides are publishers only
				setup::Role::Publisher => return Err(SessionError::RoleIncompatible(client.role, role)),
				_ => setup::Role::Subscriber,
			},
			setup::Role::Subscriber => match role {
				// Both sides are subscribers only
				setup::Role::Subscriber => return Err(SessionError::RoleIncompatible(client.role, role)),
				_ => setup::Role::Publisher,
			},
		};

		let server = setup::Server {
			role,
			version: setup::Version::DRAFT_03,
			params: Default::default(),
		};

		log::debug!("sending server SETUP: {:?}", server);

		server.encode(&mut control.0).await?;

		let publisher = server.role.is_publisher().then(|| publisher::Session::new());
		let subscriber = server.role.is_subscriber().then(|| subscriber::Session::new());

		let session = Self {
			publisher,
			subscriber,
			webtransport: session,
			control,
		};

		Ok(session)
	}

	pub async fn connect(session: webtransport_quinn::Session, role: setup::Role) -> Result<Self, SessionError> {
		let mut control = session.open_bi().await?;

		let versions: setup::Versions = [setup::Version::DRAFT_03].into();

		let client = setup::Client {
			role,
			versions: versions.clone(),
			params: Default::default(),
		};

		log::debug!("sending client SETUP: {:?}", client);
		client.encode(&mut control.0).await?;

		let server = setup::Server::decode(&mut control.1).await?;

		log::debug!("received server SETUP: {:?}", server);

		// Downgrade our role based on the server's role.
		let role = match server.role {
			setup::Role::Both => role,
			setup::Role::Publisher => match role {
				// Both sides are publishers only
				setup::Role::Publisher => return Err(SessionError::RoleIncompatible(server.role, role)),
				_ => setup::Role::Subscriber,
			},
			setup::Role::Subscriber => match role {
				// Both sides are subscribers only
				setup::Role::Subscriber => return Err(SessionError::RoleIncompatible(server.role, role)),
				_ => setup::Role::Publisher,
			},
		};

		let publisher = role.is_publisher().then(|| publisher::Session::new());
		let subscriber = role.is_subscriber().then(|| subscriber::Session::new());

		let session = Self {
			publisher,
			subscriber,
			webtransport: session,
			control,
		};

		Ok(session)
	}

	pub async fn run(&mut self) -> Result<(), SessionError> {
		loop {
			let (stream, _) = self.webtransport.accept().await?;

			let source = data::Header::decode(&mut stream).await?;

			match source {
				data::Header::Control => {
					let msg = control::Message::decode(&mut stream).await?;
					self.recv_control(msg)?;
				}
				data::Header::Stream => {
					self.recv_stream(stream).await?;
				}
			}
		}
	}

	fn recv_control(&mut self, msg: control::Message) -> Result<(), SessionError> {
		match msg.source() {
			MessageSource::Publisher => self
				.subscriber
				.as_mut()
				.ok_or(SessionError::RoleViolation)?
				.recv_control(msg),
			MessageSource::Subscriber => self
				.publisher
				.as_mut()
				.ok_or(SessionError::RoleViolation)?
				.recv_control(msg),
			MessageSource::Client => todo!("client messages"),
			MessageSource::Server => todo!("server messages"),
		}
	}

	async fn recv_stream(&mut self, mut stream: webtransport_quinn::RecvStream) -> Result<(), SessionError> {
		let header = data::Header::decode(&mut stream).await?;

		let subscriber = self.subscriber.as_mut().ok_or(SessionError::RoleViolation)?;
		subscriber.recv_stream(header, stream)
	}
}
