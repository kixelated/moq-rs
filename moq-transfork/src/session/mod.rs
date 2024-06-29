use std::future;

use crate::{coding, message, model, setup};
use futures::{stream::FuturesUnordered, StreamExt};

mod announce;
mod announced;
mod error;
mod publisher;
mod subscribe;
mod subscribed;
mod subscriber;

pub use error::*;
pub use publisher::*;
pub use subscriber::*;

use announce::*;
use announced::*;
use subscribe::*;
use subscribed::*;

#[must_use = "run() must be called"]
pub struct Session {
	webtransport: web_transport::Session,
	setup: coding::Stream,
	publisher: Option<Publisher>,
	subscriber: Option<Subscriber>,
	unknown: Option<model::UnknownWriter>, // provided to Subscriber
}

impl Session {
	fn new(
		webtransport: web_transport::Session,
		role: setup::Role,
		stream: coding::Stream,
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

	pub async fn connect(session: web_transport::Session) -> Result<(Session, Publisher, Subscriber), SessionError> {
		Self::connect_role(session, setup::Role::Both)
			.await
			.map(|(session, publisher, subscriber)| (session, publisher.unwrap(), subscriber.unwrap()))
	}

	#[tracing::instrument("connect", fields(session=session.id(), role=?role))]
	pub async fn connect_role(
		mut session: web_transport::Session,
		role: setup::Role,
	) -> Result<(Session, Option<Publisher>, Option<Subscriber>), SessionError> {
		let mut stream = coding::Stream::open(&mut session, message::Control::Session).await?;

		let role = match Self::connect_setup(&mut stream, role).await {
			Ok(role) => role,
			Err(err) => {
				tracing::error!(?err);
				stream.writer.reset(err.code());
				return Err(err);
			}
		};

		let session = Session::new(session, role, stream);
		Ok(session)
	}

	async fn connect_setup(setup: &mut coding::Stream, role: setup::Role) -> Result<setup::Role, SessionError> {
		let request = setup::Client {
			role,
			versions: [setup::Version::FORK_00].into(),
			path: None, // TODO use for QUIC
			unknown: Default::default(),
		};

		tracing::info!(?role, version = ?setup::Version::FORK_00, "sending");
		setup.writer.encode(&request).await?;

		let response: setup::Server = setup.reader.decode().await?;
		tracing::info!(?role, version = ?response.version, "received");

		// Downgrade our role based on the server's role.
		let role = match response.role {
			setup::Role::Both => role,
			setup::Role::Publisher => match role {
				// Both sides are publishers only
				setup::Role::Publisher => return Err(SessionError::RoleIncompatible(response.role, role)),
				_ => setup::Role::Subscriber,
			},
			setup::Role::Subscriber => match role {
				// Both sides are subscribers only
				setup::Role::Subscriber => return Err(SessionError::RoleIncompatible(response.role, role)),
				_ => setup::Role::Publisher,
			},
		};

		Ok(role)
	}

	pub async fn accept(
		session: web_transport::Session,
	) -> Result<(Session, Option<Publisher>, Option<Subscriber>), SessionError> {
		Self::accept_role(session, setup::Role::Both).await
	}

	#[tracing::instrument("accept", fields(session=session.id(), role=?role))]
	pub async fn accept_role(
		mut session: web_transport::Session,
		role: setup::Role,
	) -> Result<(Session, Option<Publisher>, Option<Subscriber>), SessionError> {
		let (t, mut control) = coding::Stream::accept(&mut session).await?;
		if t != message::Control::Session {
			return Err(SessionError::UnexpectedStream(t));
		}

		let role = Self::accept_setup(&mut control, role).await;
		if let Err(err) = role.as_ref() {
			tracing::error!(?err);
			control.writer.reset(err.code());
			// NOTE: returned down below via ?
		}

		Ok(Session::new(session, role?, control))
	}

	async fn accept_setup(control: &mut coding::Stream, role: setup::Role) -> Result<setup::Role, SessionError> {
		let request: setup::Client = control.reader.decode().await?;
		tracing::info!(role = ?request.role, versions = ?request.versions, "recv");

		if !request.versions.contains(&setup::Version::FORK_00) {
			return Err(SessionError::Version(
				request.versions,
				[setup::Version::FORK_00].into(),
			));
		}

		// Downgrade our role based on the client's role.
		let role = match request.role {
			setup::Role::Both => role,
			setup::Role::Publisher => match role {
				// Both sides are publishers only
				setup::Role::Publisher => return Err(SessionError::RoleIncompatible(request.role, role)),
				_ => setup::Role::Subscriber,
			},
			setup::Role::Subscriber => match role {
				// Both sides are subscribers only
				setup::Role::Subscriber => return Err(SessionError::RoleIncompatible(request.role, role)),
				_ => setup::Role::Publisher,
			},
		};

		let response = setup::Server {
			role,
			version: setup::Version::FORK_00,
			unknown: Default::default(),
		};

		tracing::info!(?role, version = ?response.version, "send");
		control.writer.encode(&response).await?;

		Ok(role)
	}

	pub async fn run(mut self) -> Result<(), SessionError> {
		let res = tokio::select! {
			res = Self::run_control(&mut self.setup) => res,
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
		};

		if let Err(err) = &res {
			tracing::error!(?err);
			self.setup.writer.reset(err.code());
		}

		res
	}

	#[tracing::instrument("run session", fields(stream = stream.id()))]
	async fn run_control(stream: &mut coding::Stream) -> Result<(), SessionError> {
		while let Some(info) = stream.reader.decode_maybe::<setup::Info>().await? {
			// TODO use info
			tracing::info!(?info);
		}

		Ok(())
	}

	async fn run_uni(
		mut webtransport: web_transport::Session,
		subscriber: Option<Subscriber>,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = webtransport.accept_uni() => {
					let mut reader = coding::Reader::new(res?);
					let mut subscriber = subscriber.clone().ok_or(SessionError::RoleViolation)?;

					tasks.push(async move {
						match reader.decode().await? {
							message::StreamUni::Group => {
								subscriber.recv_group(reader).await
							},
						};

						Ok::<(), SessionError>(())
					});
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
				res = coding::Stream::accept(&mut session) => {
					let (t, stream) = res?;
					let publisher = publisher.clone();
					let subscriber = subscriber.clone();

					tasks.push(async move {
						Ok(match t {
							message::Control::Session => return Err(SessionError::UnexpectedStream(t)),
							message::Control::Announce => {
								let mut subscriber = subscriber.ok_or(SessionError::RoleViolation)?;
								subscriber.recv_announce(stream).await;
							},
							message::Control::Subscribe => {
								let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
								publisher.recv_subscribe(stream).await;
							},
							message::Control::Datagrams => {
								let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
								publisher.recv_datagrams(stream).await;
							},
							message::Control::Fetch => {
								let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
								publisher.recv_fetch(stream).await;
							},
							message::Control::Info => {
								let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
								publisher.recv_info(stream).await;
							},
						})
					});
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?
			};
		}
	}
}
