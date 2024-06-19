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
		control: coding::Stream,
	) -> (Self, Option<Publisher>, Option<Subscriber>) {
		let unknown = model::Unknown::produce();
		let publisher = role.is_publisher().then(|| Publisher::new(webtransport.clone()));
		let subscriber = role
			.is_subscriber()
			.then(|| Subscriber::new(webtransport.clone(), unknown.1));

		let session = Self {
			webtransport,
			setup: control,
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
		let mut control = coding::Stream::open(&mut session, message::Control::Session).await?;

		let role = match Self::connect_setup(&mut control, role).await {
			Ok(role) => role,
			Err(err) => {
				control.writer.reset(err.code());
				return Err(err);
			}
		};

		let session = Session::new(session, role, control);
		Ok(session)
	}

	async fn connect_setup(setup: &mut coding::Stream, role: setup::Role) -> Result<setup::Role, SessionError> {
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

		Ok(role)
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
		let (t, mut control) = coding::Stream::accept(&mut session).await?;
		if t != message::Control::Session {
			return Err(SessionError::UnexpectedStream(t));
		}

		let role = Self::accept_setup(&mut control, role).await;
		if let Err(err) = role.as_ref() {
			control.writer.reset(err.code());
		}

		Ok(Session::new(session, role?, control))
	}

	async fn accept_setup(control: &mut coding::Stream, role: setup::Role) -> Result<setup::Role, SessionError> {
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
			log::error!("session error: {}", err);
			self.setup.writer.reset(err.code());
		}

		res
	}

	async fn run_control(mut control: &mut coding::Stream) -> Result<(), SessionError> {
		while let Some(info) = control.reader.decode_maybe::<setup::Info>().await? {
			// TODO use info
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
						let t = reader.decode().await?;
						let res = match t {
							message::StreamUni::Group => {
								subscriber.run_group(&mut reader).await
							},
						};

						if let Err(SessionError::Closed(closed)) = res.as_ref() {
							reader.stop(closed.code());
							log::warn!("closed data stream: type={:?} err={}", t, closed);
							Ok(())
						} else {
							res
						}
					});
				},
				res = tasks.next(), if !tasks.is_empty() => {
					if let Err(err) = res.unwrap() {
						log::warn!("failed to accept data stream: err={}", err);
					}
				}
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
				res = coding::Stream::accept(&mut webtransport) => {
					let (t, mut control) = res?;
					let publisher = publisher.clone();
					let subscriber = subscriber.clone();

					tasks.push(async move {
						let res = match t {
							message::Control::Session => Err(SessionError::UnexpectedStream(t)),
							message::Control::Announce => {
								let mut subscriber = subscriber.ok_or(SessionError::RoleViolation)?;
								subscriber.run_announce(&mut control).await
							},
							message::Control::Subscribe => {
								let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
								publisher.run_subscribe(&mut control).await
							},
							message::Control::Datagrams => {
								let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
								publisher.run_datagrams(&mut control).await
							},
							message::Control::Fetch => {
								let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
								publisher.run_fetch(&mut control).await
							},
							message::Control::Info => {
								let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
								publisher.run_info(&mut control).await
							},
						};

						if let Err(SessionError::Closed(closed)) = res.as_ref() {
							control.writer.reset(closed.code());
							log::warn!("closed control stream: type={:?} err={}", t, closed);
							Ok(())
						} else {
							res
						}
					});
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?
			};
		}
	}
}
