use std::future;

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	coding::{self, Reader, Stream},
	message::{self},
	setup, Publisher, ServeError, Subscriber, Unknown, UnknownWriter,
};

#[must_use = "run() must be called"]
pub struct Session {
	webtransport: web_transport::Session,

	publisher: Option<Publisher>,
	subscriber: Option<Subscriber>,
	unknown: Option<UnknownWriter>, // provided to Subscriber
}

impl Session {
	fn new(webtransport: web_transport::Session, role: setup::Role) -> (Self, Option<Publisher>, Option<Subscriber>) {
		let unknown = Unknown::produce();
		let publisher = role.is_publisher().then(|| Publisher::new(webtransport.clone()));
		let subscriber = role
			.is_subscriber()
			.then(|| Subscriber::new(webtransport.clone(), unknown.1));

		let session = Self {
			webtransport,
			publisher: publisher.clone(),
			subscriber: subscriber.clone(),
			unknown: Some(unknown.0),
		};

		(session, publisher, subscriber)
	}

	pub async fn connect(session: web_transport::Session) -> Result<(Session, Publisher, Subscriber), SessionError> {
		Self::connect_role(session, setup::Role::Both)
			.await
			.map(|(session, publisher, subscriber)| (session, publisher.unwrap(), subscriber.unwrap()))
	}

	pub async fn connect_role(
		mut session: web_transport::Session,
		role: setup::Role,
	) -> Result<(Session, Option<Publisher>, Option<Subscriber>), SessionError> {
		let mut setup = Stream::open(&mut session, message::Control::Session).await?;
		let versions: setup::Versions = [setup::Version::FORK_00].into();

		let client = setup::Client {
			role,
			versions: versions.clone(),
			path: None, // TODO use for QUIC
			unknown: Default::default(),
		};

		log::debug!("sending client setup: {:?}", client);
		setup.writer.encode(&client).await?;

		let server: setup::Server = setup.reader.decode().await?;
		log::debug!("received server setup: {:?}", server);

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

		Ok(Session::new(session, role))
	}

	pub async fn accept(
		session: web_transport::Session,
	) -> Result<(Session, Option<Publisher>, Option<Subscriber>), SessionError> {
		Self::accept_role(session, setup::Role::Both).await
	}

	pub async fn accept_role(
		mut session: web_transport::Session,
		role: setup::Role,
	) -> Result<(Session, Option<Publisher>, Option<Subscriber>), SessionError> {
		let (t, mut control) = Stream::accept(&mut session).await?;
		if t != message::Control::Session {
			return Err(SessionError::UnexpectedStream(t));
		}

		let client: setup::Client = control.reader.decode().await?;
		log::debug!("received client SETUP: {:?}", client);

		if !client.versions.contains(&setup::Version::FORK_00) {
			return Err(SessionError::Version(client.versions, [setup::Version::FORK_00].into()));
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
			version: setup::Version::FORK_00,
			unknown: Default::default(),
		};

		log::debug!("sending server SETUP: {:?}", server);
		control.writer.encode(&server).await?;

		Ok(Session::new(session, role))
	}

	pub async fn run(self) -> Result<(), SessionError> {
		tokio::select! {
			res = Self::run_bi(self.webtransport.clone(), self.publisher.clone(), self.subscriber.clone()) => res,
			res = Self::run_uni(self.webtransport.clone(), self.subscriber.clone()) => res,
			res = async move {
				match self.publisher {
					Some(publisher) => publisher.run().await,
					None => future::pending().await,
				}
			} => res,
			res = async move {
				match self.subscriber {
					Some(subscriber) => subscriber.run(self.unknown.unwrap()).await,
					None => future::pending().await,
				}
			} => res,
		}
	}

	async fn run_uni(
		mut webtransport: web_transport::Session,
		subscriber: Option<Subscriber>,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = webtransport.accept_uni() => {
					let mut reader = Reader::new(res?);
					let mut subscriber = subscriber.clone().ok_or(SessionError::RoleViolation)?;

					tasks.push(async move {
						match reader.decode().await? {
							message::StreamUni::Group =>  {
								subscriber.run_group(reader).await
							},
						}
					});
				},
				_ = tasks.next(), if !tasks.is_empty() => {},
			};
		}
	}

	async fn run_bi(
		mut webtransport: web_transport::Session,
		publisher: Option<Publisher>,
		subscriber: Option<Subscriber>,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = Stream::accept(&mut webtransport) => {
					let (t, control) = res?;
					let publisher = publisher.clone();
					let subscriber = subscriber.clone();

					tasks.push(async move {
						match t {
							message::Control::Session => Err(SessionError::UnexpectedStream(t)),
							message::Control::Announce => {
								let mut subscriber = subscriber.ok_or(SessionError::RoleViolation)?;
								subscriber.run_announce(control).await
							},
							message::Control::Subscribe => {
								let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
								publisher.run_subscribe(control).await
							},
							message::Control::Datagrams => {
								let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
								publisher.run_datagrams(control).await
							},
							message::Control::Fetch => {
								let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
								publisher.run_fetch(control).await
							},
							message::Control::Info => {
								let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
								publisher.run_info(control).await
							},
						}
					});
				},
				_ = tasks.next(), if !tasks.is_empty() => {},
			};
		}
	}
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum SessionError {
	#[error("webtransport session: {0}")]
	Session(#[from] web_transport::SessionError),

	#[error("write error: {0}")]
	Write(#[from] coding::WriteError),

	#[error("read error: {0}")]
	Read(#[from] coding::ReadError),

	// TODO move to a ConnectError
	#[error("unsupported versions: client={0:?} server={1:?}")]
	Version(setup::Versions, setup::Versions),

	// TODO move to a ConnectError
	#[error("incompatible roles: client={0:?} server={1:?}")]
	RoleIncompatible(setup::Role, setup::Role),

	/// The role negiotiated in the handshake was violated. For example, a publisher sent a SUBSCRIBE, or a subscriber sent an OBJECT.
	#[error("role violation")]
	RoleViolation,

	#[error("unexpected stream: {0:?}")]
	UnexpectedStream(message::Control),

	/// Some VarInt was too large and we were too lazy to handle it
	#[error("varint bounds exceeded")]
	BoundsExceeded(#[from] coding::BoundsExceeded),

	/// A duplicate ID was used
	#[error("duplicate")]
	Duplicate,

	#[error("internal error")]
	Internal,

	#[error("serve error: {0}")]
	Serve(#[from] ServeError),

	#[error("wrong size")]
	WrongSize,
}

impl SessionError {
	/// An integer code that is sent over the wire.
	pub fn code(&self) -> u32 {
		match self {
			Self::RoleIncompatible(..) => 406,
			Self::RoleViolation => 405,
			Self::Session(_) => 503,
			Self::Read(_) => 400,
			Self::Write(_) => 500,
			Self::Version(..) => 406,
			Self::UnexpectedStream(_) => 500,
			Self::BoundsExceeded(_) => 500,
			Self::Duplicate => 409,
			Self::Internal => 500,
			Self::WrongSize => 400,
			Self::Serve(err) => err.code(),
		}
	}
}

impl From<SessionError> for ServeError {
	fn from(err: SessionError) -> Self {
		match err {
			SessionError::Serve(err) => err,
			_ => ServeError::Internal(err.to_string()),
		}
	}
}
