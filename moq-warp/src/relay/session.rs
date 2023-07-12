use anyhow::Context;
use moq_generic_transport::{SendStream, SendStreamUnframed, BidiStream, Connection, RecvStream};

use super::{broker, contribute, control, distribute};

use moq_transport::{Role, SetupServer, Version};
use moq_transport_quinn::Connect;

pub struct Session<S: SendStream + SendStreamUnframed + Send, R: RecvStream + Send, B: BidiStream<SendStream = S, RecvStream = R>, C: Connection<SendStream = S, RecvStream = R, BidiStream = B> + Send> {
	// Split logic into contribution/distribution to reduce the problem space.
	contribute: contribute::Session<S, R, B, C>,
	distribute: distribute::Session<S, B, C>,

	// Used to receive control messages and forward to contribute/distribute.
	control: control::Main<S, B>,
}

impl<S: SendStream + SendStreamUnframed + Send + 'static, B: BidiStream<SendStream = S, RecvStream = R>, R: RecvStream + Send + 'static, C: Connection<SendStream = S, RecvStream = R, BidiStream = B> + Send + 'static> Session<S, R, B, C> {
	// pub async fn accept(session: Connect, broker: broker::Broadcasts) -> anyhow::Result<Session<S, R, B, C>> {
	// 	// Accep the WebTransport session.
	// 	// OPTIONAL validate the conn.uri() otherwise call conn.reject()
	// 	let session = session
	// 		.accept()
	// 		.await
	// 		.context(": server::Setupfailed to accept WebTransport session")?;

	// 	session
	// 		.setup()
	// 		.versions
	// 		.iter()
	// 		.find(|v| **v == Version::DRAFT_00)
	// 		.context("failed to find supported version")?;

	// 	// Choose our role based on the client's role.
	// 	let role = match session.setup().role {
	// 		Role::Publisher => Role::Subscriber,
	// 		Role::Subscriber => Role::Publisher,
	// 		Role::Both => Role::Both,
	// 	};

	// 	let setup = SetupServer {
	// 		version: Version::DRAFT_00,
	// 		role,
	// 	};

	// 	let session = session.accept(setup).await?;

	// 	let (control, objects) = session.split();
	// 	let (objects_send, objects_recv) = objects.split();

	// 	let (control, contribute, distribute) = control::split(control);

	// 	let contribute = contribute::Session::new(objects_recv, contribute, broker.clone());
	// 	let distribute = distribute::Session::new(objects_send, distribute, broker);

	// 	let session = Self {
	// 		control,
	// 		contribute,
	// 		distribute,
	// 	};

	// 	Ok(session)
	// }

	pub async fn from_session(
		session: moq_transport_trait::Session<S, R, B, C>,
		broker: broker::Broadcasts,
	) -> anyhow::Result<Session<S, R, B, C>> {
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
