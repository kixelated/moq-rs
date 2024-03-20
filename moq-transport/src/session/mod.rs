mod announce;
mod announced;
mod error;
mod publisher;
mod subscribe;
mod subscribed;
mod subscriber;

pub use announce::*;
pub use announced::*;
pub use error::*;
pub use publisher::*;
pub use subscribe::*;
pub use subscribed::*;
pub use subscriber::*;

use futures::FutureExt;
use futures::{stream::FuturesUnordered, StreamExt};
use webtransport_quinn::{RecvStream, SendStream};

use crate::{message, setup, util::Queue};

type Messages<T> = Queue<T, SessionError>;

pub struct Session {
	webtransport: webtransport_quinn::Session,
	control: (SendStream, RecvStream),

	publisher: Option<Publisher>,
	subscriber: Option<Subscriber>,
	outgoing: Messages<message::Message>,
}

impl Session {
	fn new(
		webtransport: webtransport_quinn::Session,
		control: (SendStream, RecvStream),
		role: setup::Role,
	) -> (Self, Option<Publisher>, Option<Subscriber>) {
		let outgoing = Messages::<message::Message>::default();

		let publisher = role
			.is_publisher()
			.then(|| Publisher::new(webtransport.clone(), outgoing.clone()));
		let subscriber = role.is_subscriber().then(|| Subscriber::new(outgoing.clone()));

		let session = Self {
			webtransport,
			control,
			outgoing,
			publisher: publisher.clone(),
			subscriber: subscriber.clone(),
		};

		(session, publisher, subscriber)
	}

	pub async fn connect(
		session: webtransport_quinn::Session,
	) -> Result<(Session, Option<Publisher>, Option<Subscriber>), SessionError> {
		Self::connect_role(session, setup::Role::Both).await
	}

	pub async fn connect_role(
		session: webtransport_quinn::Session,
		role: setup::Role,
	) -> Result<(Session, Option<Publisher>, Option<Subscriber>), SessionError> {
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

		Ok(Session::new(session, control, role))
	}

	pub async fn accept(
		session: webtransport_quinn::Session,
	) -> Result<(Session, Option<Publisher>, Option<Subscriber>), SessionError> {
		Self::accept_role(session, setup::Role::Both).await
	}

	pub async fn accept_role(
		session: webtransport_quinn::Session,
		role: setup::Role,
	) -> Result<(Session, Option<Publisher>, Option<Subscriber>), SessionError> {
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

		Ok(Session::new(session, control, role))
	}

	pub async fn run(self) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();
		tasks.push(Self::run_send(self.outgoing, self.control.0).boxed());
		tasks.push(Self::run_recv(self.control.1, self.publisher, self.subscriber.clone()).boxed());

		if let Some(subscriber) = self.subscriber {
			tasks.push(Self::run_streams(self.webtransport.clone(), subscriber.clone()).boxed());
			tasks.push(Self::run_datagrams(self.webtransport, subscriber).boxed());
		}

		let res = tasks.next().await.unwrap();
		Err(res.expect_err("run terminated with OK"))
	}

	async fn run_send(
		outgoing: Queue<message::Message, SessionError>,
		mut stream: SendStream,
	) -> Result<(), SessionError> {
		loop {
			let msg = outgoing.pop().await?;
			msg.encode(&mut stream).await?;
		}
	}

	async fn run_recv(
		mut stream: RecvStream,
		mut publisher: Option<Publisher>,
		mut subscriber: Option<Subscriber>,
	) -> Result<(), SessionError> {
		loop {
			let msg = message::Message::decode(&mut stream).await?;

			let msg = match TryInto::<message::Publisher>::try_into(msg) {
				Ok(msg) => {
					subscriber
						.as_mut()
						.ok_or(SessionError::RoleViolation)?
						.recv_message(msg)?;
					continue;
				}
				Err(msg) => msg,
			};

			let msg = match TryInto::<message::Subscriber>::try_into(msg) {
				Ok(msg) => {
					publisher
						.as_mut()
						.ok_or(SessionError::RoleViolation)?
						.recv_message(msg)?;
					continue;
				}
				Err(msg) => msg,
			};

			// TODO GOAWAY
			unimplemented!("unknown message context: {:?}", msg)
		}
	}

	async fn run_streams(
		webtransport: webtransport_quinn::Session,
		subscriber: Subscriber,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = webtransport.accept_uni() => {
					let stream = res?;
					tasks.push(Subscriber::recv_stream(subscriber.clone(), stream));
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?,
			};
		}
	}

	async fn run_datagrams(
		webtransport: webtransport_quinn::Session,
		mut subscriber: Subscriber,
	) -> Result<(), SessionError> {
		loop {
			let datagram = webtransport.read_datagram().await?;
			subscriber.recv_datagram(datagram).await?;
		}
	}
}
