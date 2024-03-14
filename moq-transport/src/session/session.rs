use futures::FutureExt;
use futures::{stream::FuturesUnordered, StreamExt};
use tokio::select;
use webtransport_quinn::{RecvStream, SendStream};

use crate::{
	control::{self, MessageSource},
	data,
	error::SessionError,
	publisher, setup, subscriber,
};

pub struct Session {
	webtransport: webtransport_quinn::Session,
	control: (SendStream, RecvStream),

	publisher: Option<publisher::Session>,
	subscriber: Option<subscriber::Session>,
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
			webtransport: session,
			publisher,
			subscriber,
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
			webtransport: session,
			publisher,
			subscriber,
			control,
		};

		Ok(session)
	}

	async fn run(mut self) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		tasks.push(Self::send_messages(self.control.0, self.publisher.clone(), self.subscriber.clone()).boxed());
		tasks.push(Self::recv_messages(self.control.1, self.publisher, self.subscriber.clone()).boxed());
		tasks.push(Self::recv_streams(self.webtransport.clone(), self.subscriber).boxed());

		let res: Option<Result<(), SessionError>> = tasks.next().await;
		let err = res.unwrap().err().unwrap_or(SessionError::Internal);
		self.webtransport.close(err.code() as u32, err.to_string().as_bytes());

		Err(err)
	}

	async fn send_messages(
		mut control: SendStream,
		mut publisher: Option<publisher::Session>,
		mut subscriber: Option<subscriber::Session>,
	) -> Result<(), SessionError> {
		loop {
			let msg = select! {
				res = publisher.as_mut().unwrap().next_message(), if publisher.is_some() => res?,
				res = subscriber.as_mut().unwrap().next_message(), if subscriber.is_some() => res?,
			};

			msg.encode(&mut control).await?;
		}
	}

	async fn recv_messages(
		mut control: RecvStream,
		mut publisher: Option<publisher::Session>,
		mut subscriber: Option<subscriber::Session>,
	) -> Result<(), SessionError> {
		loop {
			let msg = control::Message::decode(&mut control).await?;
			match msg.source() {
				MessageSource::Publisher => subscriber
					.as_mut()
					.ok_or(SessionError::RoleViolation)?
					.recv_message(msg),
				MessageSource::Subscriber => publisher.as_mut().ok_or(SessionError::RoleViolation)?.recv_message(msg),
				MessageSource::Client => todo!("client messages"),
				MessageSource::Server => todo!("server messages"),
			}?;
		}
	}

	async fn recv_streams(
		webtransport: webtransport_quinn::Session,
		subscriber: Option<subscriber::Session>,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			// TODO use futures instead
			tokio::select! {
				res = webtransport.accept_uni() => {
					let stream = res?;
					let subscriber = subscriber.clone().ok_or(SessionError::RoleViolation)?;
					tasks.push(Self::recv_stream(stream, subscriber));
				},
				res = tasks.next(), if tasks.len() > 0 => res.unwrap()?,
			};
		}
	}

	async fn recv_stream(
		mut stream: webtransport_quinn::RecvStream,
		mut subscriber: subscriber::Session,
	) -> Result<(), SessionError> {
		let header = data::Header::decode(&mut stream).await?;
		subscriber.recv_stream(header, stream)
	}
}
