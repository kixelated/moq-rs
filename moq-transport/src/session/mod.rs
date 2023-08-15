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
		let (send, recv) = session.accept_bi().await.context("failed to accept bidi stream")?;

		let mut send_control = message::Sender::new(send);
		let mut recv_control = message::Receiver::new(recv);

		let setup_client = match recv_control.recv().await.context("failed to read SETUP")? {
			message::Message::SetupClient(setup) => setup,
			_ => anyhow::bail!("expected CLIENT SETUP"),
		};

		setup_client
			.versions
			.iter()
			.find(|version| **version == setup::Version::DRAFT_00)
			.context("no supported versions")?;

		if !setup_client.role.compatible(role) {
			anyhow::bail!("incompatible roles: {:?} {:?}", setup_client.role, role);
		}

		let setup_server = setup::Server {
			role,
			version: setup::Version::DRAFT_00,
		};

		send_control
			.send(message::Message::SetupServer(setup_server))
			.await
			.context("failed to send setup server")?;

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
		let (send, recv) = session.open_bi().await.context("failed to oen bidi stream")?;

		let mut send_control = message::Sender::new(send);
		let mut recv_control = message::Receiver::new(recv);

		let setup_client = setup::Client {
			role,
			versions: vec![setup::Version::DRAFT_00].into(),
			path: "".to_string(),
		};

		send_control
			.send(message::Message::SetupClient(setup_client))
			.await
			.context("failed to send SETUP CLIENT")?;

		let setup_server = match recv_control.recv().await.context("failed to read SETUP")? {
			message::Message::SetupServer(setup) => setup,
			_ => anyhow::bail!("expected SERVER SETUP"),
		};

		if setup_server.version != setup::Version::DRAFT_00 {
			anyhow::bail!("unsupported version: {:?}", setup_server.version);
		}

		if !setup_server.role.compatible(role) {
			anyhow::bail!("incompatible roles: {:?} {:?}", role, setup_server.role);
		}

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
