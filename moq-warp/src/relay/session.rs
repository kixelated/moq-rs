use anyhow::Context;

use std::sync::Arc;

use moq_transport::{control, server, setup};

use super::{broker, Contribute, Distribute};

pub struct Session {
	// Used to receive control messages.
	control: control::RecvStream,

	// Split logic into contribution/distribution to reduce the problem space.
	contribute: Contribute,
	distribute: Distribute,
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

		// Split the control stream into send/receive halves.
		let (sender, receiver) = control.split();
		let sender = sender.share(); // wrap in an arc/mutex

		let contribute = Contribute::new(transport.clone(), sender.clone(), broker.clone());
		let distribute = Distribute::new(transport, sender, broker);

		let session = Self {
			control: receiver,
			contribute,
			distribute,
		};

		Ok(session)
	}

	pub async fn run(&mut self) -> anyhow::Result<()> {
		loop {
			tokio::select! {
				msg = self.control.recv() => {
					let msg = msg.context("failed to receive control message")?;
					self.receive_message(msg).await?;
				},
				res = self.contribute.run() => {
					res.context("failed to run contribution")?;
				},
				res = self.distribute.run() => {
					res.context("failed to run contribution")?;
				},
			}
		}
	}

	async fn receive_message(&mut self, msg: control::Message) -> anyhow::Result<()> {
		// TODO split messages into contribution/distribution types to make this safer.
		match msg {
			control::Message::Announce(_) | control::Message::SubscribeOk(_) | control::Message::SubscribeError(_) => {
				self.contribute.receive_message(msg).await
			}
			control::Message::AnnounceOk(_) | control::Message::AnnounceError(_) | control::Message::Subscribe(_) => {
				self.distribute.receive_message(msg).await
			}
			control::Message::GoAway(_) => anyhow::bail!("client can't send GOAWAY u nerd"),
		}
	}
}
