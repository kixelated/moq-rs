mod announce;
mod announced;
mod error;
mod publisher;
mod reader;
mod subscribe;
mod subscribed;
mod subscriber;
mod writer;

pub use announce::*;
pub use announced::*;
pub use error::*;
pub use publisher::*;
pub use subscribe::*;
pub use subscribed::*;
pub use subscriber::*;

use reader::*;
use writer::*;

use futures::FutureExt;
use futures::{stream::FuturesUnordered, StreamExt};

use crate::message::Message;
use crate::watch::Queue;
use crate::{message, setup};

#[must_use = "run() must be called"]
pub struct Session {
	webtransport: web_transport::Session,

	sender: Writer,
	recver: Reader,

	publisher: Option<Publisher>,
	subscriber: Option<Subscriber>,

	outgoing: Queue<Message>,
}

impl Session {
	fn new(
		webtransport: web_transport::Session,
		sender: Writer,
		recver: Reader,
		role: setup::Role,
	) -> (Self, Option<Publisher>, Option<Subscriber>) {
		let outgoing = Queue::default().split();
		let publisher = role
			.is_publisher()
			.then(|| Publisher::new(outgoing.0.clone(), webtransport.clone()));
		let subscriber = role.is_subscriber().then(|| Subscriber::new(outgoing.0));

		let session = Self {
			webtransport,
			sender,
			recver,
			publisher: publisher.clone(),
			subscriber: subscriber.clone(),
			outgoing: outgoing.1,
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
		let control = session.open_bi().await?;
		let mut sender = Writer::new(control.0);
		let mut recver = Reader::new(control.1);

		let versions: setup::Versions = [setup::Version::DRAFT_03].into();

		let client = setup::Client {
			role,
			versions: versions.clone(),
			params: Default::default(),
		};

		log::debug!("sending client SETUP: {:?}", client);
		sender.encode(&client).await?;

		let server: setup::Server = recver.decode().await?;
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

		Ok(Session::new(session, sender, recver, role))
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
		let control = session.accept_bi().await?;
		let mut sender = Writer::new(control.0);
		let mut recver = Reader::new(control.1);

		let client: setup::Client = recver.decode().await?;
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
		sender.encode(&server).await?;

		Ok(Session::new(session, sender, recver, role))
	}

	pub async fn run(self) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		tasks.push(Self::run_recv(self.recver, self.publisher, self.subscriber.clone()).boxed_local());
		tasks.push(Self::run_send(self.sender, self.outgoing).boxed_local());

		if let Some(subscriber) = self.subscriber {
			tasks.push(Self::run_streams(self.webtransport.clone(), subscriber.clone()).boxed_local());
			tasks.push(Self::run_datagrams(self.webtransport, subscriber).boxed_local());
		}

		let res = tasks.select_next_some().await;
		Err(res.expect_err("run terminated with OK"))
	}

	async fn run_send(mut sender: Writer, mut outgoing: Queue<message::Message>) -> Result<(), SessionError> {
		while let Some(msg) = outgoing.pop().await {
			log::debug!("sending message: {:?}", msg);
			sender.encode(&msg).await?;
		}

		Ok(())
	}

	async fn run_recv(
		mut recver: Reader,
		mut publisher: Option<Publisher>,
		mut subscriber: Option<Subscriber>,
	) -> Result<(), SessionError> {
		loop {
			let msg: message::Message = recver.decode().await?;
			log::debug!("received message: {:?}", msg);

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

	async fn run_streams(mut webtransport: web_transport::Session, subscriber: Subscriber) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = webtransport.accept_uni() => {
					let stream = res?;
					let subscriber = subscriber.clone();

					tasks.push(async move {
						if let Err(err) = Subscriber::recv_stream(subscriber, stream).await {
							log::warn!("failed to serve stream: {}", err);
						};
					});
				},
				_ = tasks.next(), if !tasks.is_empty() => {},
			};
		}
	}

	async fn run_datagrams(
		mut webtransport: web_transport::Session,
		mut subscriber: Subscriber,
	) -> Result<(), SessionError> {
		loop {
			let datagram = webtransport.recv_datagram().await?;
			subscriber.recv_datagram(datagram)?;
		}
	}
}
