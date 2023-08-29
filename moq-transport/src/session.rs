use anyhow::Context;

use crate::{message, object, setup};
use webtransport_generic::Session as WTSession;

pub struct Session<S: WTSession> {
	pub send_control: message::Sender<S::SendStream>,
	pub recv_control: message::Receiver<S::RecvStream>,
	pub send_objects: object::Sender<S>,
	pub recv_objects: object::Receiver<S>,
}

impl<S: WTSession> Session<S> {
	/// Called by a server with an established WebTransport session.
	// TODO close the session with an error code
	pub async fn accept(session: S, role: setup::Role) -> anyhow::Result<Self> {
		let (mut send, mut recv) = session.accept_bi().await.context("failed to accept bidi stream")?;

		let setup_client = setup::Client::decode(&mut recv)
			.await
			.context("failed to read CLIENT SETUP")?;

		setup_client
			.versions
			.iter()
			.find(|version| **version == setup::Version::DRAFT_00)
			.context("no supported versions")?;

		let setup_server = setup::Server {
			role,
			version: setup::Version::DRAFT_00,
		};

		setup_server
			.encode(&mut send)
			.await
			.context("failed to send setup server")?;

		let send_control = message::Sender::new(send);
		let recv_control = message::Receiver::new(recv);

		let send_objects = object::Sender::new(session.clone());
		let recv_objects = object::Receiver::new(session.clone());

		Ok(Session {
			send_control,
			recv_control,
			send_objects,
			recv_objects,
		})
	}

	/// Called by a client with an established WebTransport session.
	pub async fn connect(session: S, role: setup::Role) -> anyhow::Result<Self> {
		let (mut send, mut recv) = session.open_bi().await.context("failed to oen bidi stream")?;

		let setup_client = setup::Client {
			role,
			versions: vec![setup::Version::DRAFT_00].into(),
			path: "".to_string(),
		};

		setup_client
			.encode(&mut send)
			.await
			.context("failed to send SETUP CLIENT")?;

		let setup_server = setup::Server::decode(&mut recv).await.context("failed to read SETUP")?;

		if setup_server.version != setup::Version::DRAFT_00 {
			anyhow::bail!("unsupported version: {:?}", setup_server.version);
		}

		let send_control = message::Sender::new(send);
		let recv_control = message::Receiver::new(recv);

		let send_objects = object::Sender::new(session.clone());
		let recv_objects = object::Receiver::new(session.clone());

		Ok(Session {
			send_control,
			recv_control,
			send_objects,
			recv_objects,
		})
	}
}
