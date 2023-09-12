use crate::session::{Publisher, Subscriber};
use crate::setup;
use webtransport_quinn::{RecvStream, SendStream, Session};

use anyhow::Context;

pub struct Client {}

impl Client {
	pub async fn publisher(session: Session) -> anyhow::Result<Publisher> {
		let control = Self::send_setup(&session, setup::Role::Publisher).await?;

		let publisher = Publisher::new(session, control);
		Ok(publisher)
	}

	pub async fn subscriber(session: Session) -> anyhow::Result<Subscriber> {
		let control = Self::send_setup(&session, setup::Role::Subscriber).await?;

		let subscriber = Subscriber::new(session, control);
		Ok(subscriber)
	}

	// TODO support performing both roles
	/*
		pub async fn connect(self) -> anyhow::Result<(Publisher, Subscriber)> {
			self.connect_role(setup::Role::Both).await
		}
	*/

	async fn send_setup(session: &Session, role: setup::Role) -> anyhow::Result<(SendStream, RecvStream)> {
		let mut control = session.open_bi().await.context("failed to oen bidi stream")?;

		let client = setup::Client {
			role,
			versions: vec![setup::Version::DRAFT_00].into(),
			path: "".to_string(),
		};

		client
			.encode(&mut control.0)
			.await
			.context("failed to send SETUP CLIENT")?;

		let server = setup::Server::decode(&mut control.1)
			.await
			.context("failed to read SETUP")?;

		if server.version != setup::Version::DRAFT_00 {
			anyhow::bail!("unsupported version: {:?}", server.version);
		}

		// Make sure the server replied with the
		if !client.role.is_compatible(server.role) {
			anyhow::bail!("incompatible roles: client={:?} server={:?}", client.role, server.role);
		}

		Ok(control)
	}
}
