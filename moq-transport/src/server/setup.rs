use crate::coding::{Decode, Encode};
use crate::{control, setup};

use anyhow::Context;
use bytes::Bytes;

pub(crate) struct RecvSetup {
	stream: h3_webtransport::stream::BidiStream<h3_quinn::BidiStream<Bytes>, Bytes>,
}

impl RecvSetup {
	pub fn new(stream: h3_webtransport::stream::BidiStream<h3_quinn::BidiStream<Bytes>, Bytes>) -> Self {
		Self { stream }
	}

	pub async fn recv(mut self) -> anyhow::Result<SendSetup> {
		let setup = setup::Client::decode(&mut self.stream)
			.await
			.context("failed to read client SETUP message")?;

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
		setup.encode(&mut self.stream).await?;
		Ok(control::Stream::new(self.stream))
	}
}
