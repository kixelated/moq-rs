mod announce;
mod announced;
mod error;
mod publisher;
mod subscribe;
mod subscribed;
mod subscriber;

use std::sync::Arc;

pub use announce::*;
pub use announced::*;
pub use error::*;
pub use publisher::*;
pub use subscribe::*;
pub use subscribed::*;
pub use subscriber::*;

use futures::FutureExt;
use futures::{stream::FuturesUnordered, StreamExt};

use crate::coding::{Reader, Writer};
use crate::{message, setup, util::Queue};

pub struct Session<S: webtransport_generic::Session> {
	webtransport: S,

	sender: Writer<S::SendStream>,
	recver: Reader<S::RecvStream>,
	outgoing: Queue<message::Message, SessionError>,

	publisher: Option<Publisher<S>>,
	subscriber: Option<Subscriber<S>>,
}

impl<S: webtransport_generic::Session> Session<S> {
	fn new(
		webtransport: S,
		sender: Writer<S::SendStream>,
		recver: Reader<S::RecvStream>,
		role: setup::Role,
	) -> (Self, Option<Publisher<S>>, Option<Subscriber<S>>) {
		let outgoing = Queue::default();

		let publisher = role
			.is_publisher()
			.then(|| Publisher::new(webtransport.clone(), outgoing.clone()));
		let subscriber = role.is_subscriber().then(|| Subscriber::new(outgoing.clone()));

		let session = Self {
			webtransport,
			sender,
			recver,
			outgoing,
			publisher: publisher.clone(),
			subscriber: subscriber.clone(),
		};

		(session, publisher, subscriber)
	}

	pub async fn connect(
		session: S,
	) -> Result<(Session<S>, Option<Publisher<S>>, Option<Subscriber<S>>), SessionError> {
		Self::connect_role(session, setup::Role::Both).await
	}

	pub async fn connect_role(
		session: S,
		role: setup::Role,
	) -> Result<(Session<S>, Option<Publisher<S>>, Option<Subscriber<S>>), SessionError> {
		let control = session
			.open_bi()
			.await
			.map_err(|e| SessionError::WebTransport(Arc::new(e)))?;
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

	pub async fn accept(session: S) -> Result<(Session<S>, Option<Publisher<S>>, Option<Subscriber<S>>), SessionError> {
		Self::accept_role(session, setup::Role::Both).await
	}

	pub async fn accept_role(
		session: S,
		role: setup::Role,
	) -> Result<(Session<S>, Option<Publisher<S>>, Option<Subscriber<S>>), SessionError> {
		let control = session
			.accept_bi()
			.await
			.map_err(|e| SessionError::WebTransport(Arc::new(e)))?;
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
		tasks.push(Self::run_send(self.outgoing, self.sender).boxed());
		tasks.push(Self::run_recv(self.recver, self.publisher, self.subscriber.clone()).boxed());

		if let Some(subscriber) = self.subscriber {
			tasks.push(Self::run_streams(self.webtransport.clone(), subscriber.clone()).boxed());
			tasks.push(Self::run_datagrams(self.webtransport, subscriber).boxed());
		}

		let res = tasks.next().await.unwrap();
		Err(res.expect_err("run terminated with OK"))
	}

	async fn run_send(
		outgoing: Queue<message::Message, SessionError>,
		mut sender: Writer<S::SendStream>,
	) -> Result<(), SessionError> {
		loop {
			let msg = outgoing.pop().await?;
			sender.encode(&msg).await?;
		}
	}

	async fn run_recv(
		mut recver: Reader<S::RecvStream>,
		mut publisher: Option<Publisher<S>>,
		mut subscriber: Option<Subscriber<S>>,
	) -> Result<(), SessionError> {
		loop {
			let msg: message::Message = recver.decode().await?;

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

	async fn run_streams(webtransport: S, subscriber: Subscriber<S>) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = webtransport.accept_uni() => {
					let stream = res.map_err(|e| SessionError::WebTransport(Arc::new(e)))?;
					tasks.push(Subscriber::recv_stream(subscriber.clone(), stream));
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?,
			};
		}
	}

	async fn run_datagrams(webtransport: S, mut subscriber: Subscriber<S>) -> Result<(), SessionError> {
		loop {
			let datagram = webtransport
				.recv_datagram()
				.await
				.map_err(|e| SessionError::WebTransport(Arc::new(e)))?;

			subscriber.recv_datagram(datagram).await?;
		}
	}
}
