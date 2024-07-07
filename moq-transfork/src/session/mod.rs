use std::future;

use crate::{
	message, model,
	setup::{self, Extensions},
};
use futures::{stream::FuturesUnordered, StreamExt};

mod announce;
mod error;
mod publisher;
mod reader;
mod stream;
mod subscribe;
mod subscriber;
mod writer;

pub use error::*;
pub use publisher::*;
pub use subscriber::*;

use announce::*;
use reader::*;
use stream::*;
use subscribe::*;
use writer::*;

#[must_use = "run() must be called"]
pub struct Session {
	webtransport: web_transport::Session,
	setup: Stream,
	publisher: Option<Publisher>,
	subscriber: Option<Subscriber>,
	unknown: Option<model::UnknownWriter>, // provided to Subscriber
}

impl Session {
	fn new(
		webtransport: web_transport::Session,
		role: setup::Role,
		stream: Stream,
	) -> (Self, Option<Publisher>, Option<Subscriber>) {
		let unknown = model::Unknown::produce();
		let publisher = role.is_publisher().then(|| Publisher::new(webtransport.clone()));
		let subscriber = role
			.is_subscriber()
			.then(|| Subscriber::new(webtransport.clone(), unknown.1));

		let session = Self {
			webtransport,
			setup: stream,
			publisher: publisher.clone(),
			subscriber: subscriber.clone(),
			unknown: Some(unknown.0),
		};

		(session, publisher, subscriber)
	}

	/// Connect a session as both a publisher and subscriber.
	pub async fn connect(session: web_transport::Session) -> Result<(Session, Publisher, Subscriber), SessionError> {
		Self::connect_role(session, setup::Role::Both)
			.await
			.map(|(session, publisher, subscriber)| (session, publisher.unwrap(), subscriber.unwrap()))
	}

	/// Connect a session as either a publisher, subscriber, or both, as chosen by server.
	pub async fn connect_any(
		session: web_transport::Session,
	) -> Result<(Session, Option<Publisher>, Option<Subscriber>), SessionError> {
		Self::connect_role(session, setup::Role::Any).await
	}

	#[tracing::instrument("session", skip_all, err, fields(id = session.id))]
	pub async fn connect_role(
		mut session: web_transport::Session,
		role: setup::Role,
	) -> Result<(Session, Option<Publisher>, Option<Subscriber>), SessionError> {
		let mut stream = Stream::open(&mut session, message::Stream::Session).await?;

		let role = Self::connect_setup(&mut stream, role).await.or_close(&mut stream)?;
		Ok(Session::new(session, role, stream))
	}

	async fn connect_setup(setup: &mut Stream, client_role: setup::Role) -> Result<setup::Role, SessionError> {
		let mut extensions = setup::Extensions::default();
		extensions.set(client_role)?;

		let client = setup::Client {
			versions: [setup::Version::FORK_00].into(),
			extensions,
		};

		tracing::info!(versions = ?client.versions, role=?client_role, "client setup");
		setup.writer.encode(&client).await?;

		let server: setup::Server = setup.reader.decode().await?;
		let server_role = server.extensions.get()?.unwrap_or_default();

		tracing::info!(version = ?server.version, role=?server_role, "server setup");

		let role = client_role
			.downgrade(server_role)
			.ok_or(SessionError::RoleIncompatible(client_role, server_role))?;

		if client_role != role {
			tracing::info!(?role, "client downgraded");
		}

		Ok(role)
	}

	/// Accept a session as both a publisher and subscriber.
	pub async fn accept(session: web_transport::Session) -> Result<(Session, Publisher, Subscriber), SessionError> {
		Self::accept_role(session, setup::Role::Both)
			.await
			.map(|(session, publisher, subscriber)| (session, publisher.unwrap(), subscriber.unwrap()))
	}

	/// Accept a session as either a publisher, subscriber, or both, as chosen by the client.
	pub async fn accept_any(
		session: web_transport::Session,
	) -> Result<(Session, Option<Publisher>, Option<Subscriber>), SessionError> {
		Self::accept_role(session, setup::Role::Any).await
	}

	#[tracing::instrument("session", skip_all, err, fields(id = session.id))]
	pub async fn accept_role(
		mut session: web_transport::Session,
		role: setup::Role,
	) -> Result<(Session, Option<Publisher>, Option<Subscriber>), SessionError> {
		let mut stream = Stream::accept(&mut session).await?;
		let kind = stream.reader.decode_silent().await?;

		if kind != message::Stream::Session {
			return Err(SessionError::UnexpectedStream(kind));
		}

		let role = Self::accept_setup(&mut stream, role).await.or_close(&mut stream)?;

		Ok(Session::new(session, role, stream))
	}

	async fn accept_setup(control: &mut Stream, server_role: setup::Role) -> Result<setup::Role, SessionError> {
		let client: setup::Client = control.reader.decode().await?;

		if !client.versions.contains(&setup::Version::FORK_00) {
			return Err(SessionError::Version(client.versions, [setup::Version::FORK_00].into()));
		}

		let client_role = client.extensions.get()?.unwrap_or_default();

		tracing::info!(versions=?client.versions, role=?client_role,  "client setup");

		let server_role = server_role
			.downgrade(client_role)
			.ok_or(SessionError::RoleIncompatible(client_role, server_role))?;

		let mut extensions = Extensions::default();
		extensions.set(server_role)?;

		let server = setup::Server {
			version: setup::Version::FORK_00,
			extensions,
		};

		tracing::info!(version = ?server.version, role = ?server_role, "server setup");

		control.writer.encode(&server).await?;

		Ok(server_role)
	}

	#[tracing::instrument("session", skip_all, err, fields(id = self.webtransport.id))]
	pub async fn run(mut self) -> Result<(), SessionError> {
		tokio::select! {
			res = Self::run_update(&mut self.setup) => res,
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
		.or_close(&mut self.setup)
	}

	async fn run_update(stream: &mut Stream) -> Result<(), SessionError> {
		while let Some(_update) = stream.reader.decode_maybe::<setup::Info>().await? {}

		Ok(())
	}

	async fn run_uni(
		mut webtransport: web_transport::Session,
		subscriber: Option<Subscriber>,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = Reader::accept(&mut webtransport) => {
					let stream = res?;
					let subscriber = subscriber.clone().ok_or(SessionError::RoleViolation)?;

					tasks.push(Self::run_data(stream, subscriber));
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?,
			};
		}
	}

	async fn run_bi(
		mut session: web_transport::Session,
		publisher: Option<Publisher>,
		subscriber: Option<Subscriber>,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = Stream::accept(&mut session) => {
					let stream = res?;
					let publisher = publisher.clone();
					let subscriber = subscriber.clone();

					tasks.push(Self::run_control(stream, publisher, subscriber));
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?,
			};
		}
	}

	async fn run_data(mut stream: Reader, mut subscriber: Subscriber) -> Result<(), SessionError> {
		match stream.decode_silent().await? {
			message::StreamUni::Group => subscriber.recv_group(&mut stream).await,
		}
		.or_close(&mut stream)
		.ok();

		Ok(())
	}

	async fn run_control(
		mut stream: Stream,
		publisher: Option<Publisher>,
		subscriber: Option<Subscriber>,
	) -> Result<(), SessionError> {
		let kind = stream.reader.decode_silent().await?;
		match kind {
			message::Stream::Session => return Err(SessionError::UnexpectedStream(kind)),
			message::Stream::Announce => {
				let mut subscriber = subscriber.ok_or(SessionError::RoleViolation)?;
				subscriber.recv_announce(&mut stream).await
			}
			message::Stream::Subscribe => {
				let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
				publisher.recv_subscribe(&mut stream).await
			}
			message::Stream::Datagrams => {
				let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
				publisher.recv_datagrams(&mut stream).await
			}
			message::Stream::Fetch => {
				let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
				publisher.recv_fetch(&mut stream).await
			}
			message::Stream::Info => {
				let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
				publisher.recv_info(&mut stream).await
			}
		}
		.or_close(&mut stream)
		.ok();

		Ok(())
	}
}
