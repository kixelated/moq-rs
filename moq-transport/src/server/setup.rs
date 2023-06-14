use crate::coding::{Decode, Encode};
use crate::{control, setup};

use bytes::Bytes;

pub(crate) struct RecvSetup {
	stream: h3_webtransport::stream::BidiStream<h3_quinn::BidiStream<Bytes>, Bytes>,
}

impl RecvSetup {
	pub fn new(stream: h3_webtransport::stream::BidiStream<h3_quinn::BidiStream<Bytes>, Bytes>) -> Self {
		Self { stream }
	}

	pub async fn recv(mut self) -> anyhow::Result<SendSetup> {
		let msg = setup::Message::decode(&mut self.stream).await?;
		let setup = match msg {
			setup::Message::Client(setup) => setup,
			_ => anyhow::bail!("expected client SETUP"),
		};

		Ok(SendSetup::new(self.stream, setup))
	}
}

pub(crate) struct SendSetup {
	pub client: setup::Client,
	stream: h3_webtransport::stream::BidiStream<h3_quinn::BidiStream<Bytes>, Bytes>,
}

impl SendSetup {
	pub fn new(
		stream: h3_webtransport::stream::BidiStream<h3_quinn::BidiStream<Bytes>, Bytes>,
		client: setup::Client,
	) -> Self {
		Self { stream, client }
	}

	pub async fn send(mut self, setup: setup::Server) -> anyhow::Result<control::Stream> {
		let msg = setup::Message::Server(setup);
		msg.encode(&mut self.stream).await?;

		Ok(control::Stream::new(self.stream))
	}
}
