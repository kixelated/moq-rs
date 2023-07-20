use anyhow::Context;

use moq_transport::{Message, SetupClient, SetupServer};

use super::{RecvControl, RecvObjects, SendControl, SendObjects};

/// Called by a server with an established WebTransport session.
// TODO close the session with an error code
pub async fn accept(session: webtransport_quinn::Session, role: moq_transport::Role) -> anyhow::Result<Session> {
	let (send, recv) = session.accept_bi().await.context("failed to accept bidi stream")?;

	let mut send_control = SendControl::new(send);
	let mut recv_control = RecvControl::new(recv);

	let setup_client = match recv_control.recv().await.context("failed to read SETUP")? {
		Message::SetupClient(setup) => setup,
		_ => anyhow::bail!("expected CLIENT SETUP"),
	};

	setup_client
		.versions
		.iter()
		.find(|version| **version == moq_transport::Version::DRAFT_00)
		.context("no supported versions")?;

	if !setup_client.role.compatible(role) {
		anyhow::bail!("incompatible roles: {:?} {:?}", setup_client.role, role);
	}

	let setup_server = SetupServer {
		role,
		version: moq_transport::Version::DRAFT_00,
	};

	send_control
		.send(moq_transport::Message::SetupServer(setup_server))
		.await
		.context("failed to send setup server")?;

	let send_objects = SendObjects::new(session.clone());
	let recv_objects = RecvObjects::new(session.clone());

	Ok(Session {
		send_control,
		recv_control,
		send_objects,
		recv_objects,
	})
}

/// Called by a client with an established WebTransport session.
pub async fn connect(session: webtransport_quinn::Session, role: moq_transport::Role) -> anyhow::Result<Session> {
	let (send, recv) = session.open_bi().await.context("failed to oen bidi stream")?;

	let mut send_control = SendControl::new(send);
	let mut recv_control = RecvControl::new(recv);

	let setup_client = SetupClient {
		role,
		versions: vec![moq_transport::Version::DRAFT_00].into(),
		path: "".to_string(),
	};

	send_control
		.send(moq_transport::Message::SetupClient(setup_client))
		.await
		.context("failed to send SETUP CLIENT")?;

	let setup_server = match recv_control.recv().await.context("failed to read SETUP")? {
		Message::SetupServer(setup) => setup,
		_ => anyhow::bail!("expected SERVER SETUP"),
	};

	if setup_server.version != moq_transport::Version::DRAFT_00 {
		anyhow::bail!("unsupported version: {:?}", setup_server.version);
	}

	if !setup_server.role.compatible(role) {
		anyhow::bail!("incompatible roles: {:?} {:?}", role, setup_server.role);
	}

	let send_objects = SendObjects::new(session.clone());
	let recv_objects = RecvObjects::new(session.clone());

	Ok(Session {
		send_control,
		recv_control,
		send_objects,
		recv_objects,
	})
}

pub struct Session {
	pub send_control: SendControl,
	pub recv_control: RecvControl,
	pub send_objects: SendObjects,
	pub recv_objects: RecvObjects,
}
