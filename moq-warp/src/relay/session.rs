use anyhow::Context;

use std::sync::Arc;

use moq_transport::{server, setup};

use super::{broker, contribute, control, distribute};

pub struct Session {
	// Split logic into contribution/distribution to reduce the problem space.
	contribute: contribute::Session,
	distribute: distribute::Session,

	// Used to receive control messages and forward to contribute/distribute.
	control: control::Main,
}

impl Session {
	pub async fn accept(session: server::Accept, broker: broker::Broadcasts) -> anyhow::Result<Session> {
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
			.find(|v| **v == setup::Version::DRAFT_00)
			.context("failed to find supported version")?;

		match session.setup().role {
			setup::Role::Subscriber => {}
			_ => anyhow::bail!("TODO publishing not yet supported"),
		}

		let setup = setup::Server {
			version: setup::Version::DRAFT_00,
			role: setup::Role::Publisher,
		};

		let (transport, control) = session.accept(setup).await?;
		let transport = Arc::new(transport);

		let (control, contribute, distribute) = control::split(control);

		let contribute = contribute::Session::new(transport.clone(), contribute, broker.clone());
		let distribute = distribute::Session::new(transport, distribute, broker);

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
