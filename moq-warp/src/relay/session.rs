use anyhow::Context;

use super::{broker, contribute, control, distribute};

use moq_transport::{Role, SetupServer, Version};
use moq_transport_quinn::Connect;

pub struct Session {
	// Split logic into contribution/distribution to reduce the problem space.
	contribute: contribute::Session,
	distribute: distribute::Session,

	// Used to receive control messages and forward to contribute/distribute.
	control: control::Main,
}

impl Session {
	pub async fn accept(session: Connect, broker: broker::Broadcasts) -> anyhow::Result<Session> {
		// Accep the WebTransport session.
		// OPTIONAL validate the conn.uri() otherwise call conn.reject()
		let session = session
			.accept()
			.await
			.context(": server::Setupfailed to accept WebTransport session")?;

		session
			.setup()
			.versions
			.iter()
			.find(|v| **v == Version::DRAFT_00)
			.context("failed to find supported version")?;

		// Choose our role based on the client's role.
		let role = match session.setup().role {
			Role::Publisher => Role::Subscriber,
			Role::Subscriber => Role::Publisher,
			Role::Both => Role::Both,
		};

		let setup = SetupServer {
			version: Version::DRAFT_00,
			role,
		};

		let session = session.accept(setup).await?;

		let (control, objects) = session.split();
		let (objects_send, objects_recv) = objects.split();

		let (control, contribute, distribute) = control::split(control);

		let contribute = contribute::Session::new(objects_recv, contribute, broker.clone());
		let distribute = distribute::Session::new(objects_send, distribute, broker);

		let session = Self {
			control,
			contribute,
			distribute,
		};

		Ok(session)
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let control = self.control.run();
		let contribute = self.contribute.run();
		let distribute = self.distribute.run();

		tokio::try_join!(control, contribute, distribute)?;

		Ok(())
	}
}
