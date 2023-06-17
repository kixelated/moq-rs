use super::Header;
use anyhow::Context;
use bytes::Bytes;

use crate::coding::{Decode, Encode};

// TODO support clients
type WebTransportSession = h3_webtransport::server::WebTransportSession<h3_quinn::Connection, Bytes>;

// Reduce some typing for implementors.
pub type SendStream = h3_webtransport::stream::SendStream<h3_quinn::SendStream<Bytes>, Bytes>;
pub type RecvStream = h3_webtransport::stream::RecvStream<h3_quinn::RecvStream, Bytes>;

pub struct Transport {
	transport: WebTransportSession,
}

impl Transport {
	pub fn new(transport: WebTransportSession) -> Self {
		Self { transport }
	}

	pub async fn recv(&self) -> anyhow::Result<(Header, RecvStream)> {
		let (_session_id, mut stream) = self
			.transport
			.accept_uni()
			.await
			.context("failed to accept uni stream")?
			.context("no uni stream")?;

		let header = Header::decode(&mut stream).await?;

		Ok((header, stream))
	}

	pub async fn send(&self, header: Header) -> anyhow::Result<SendStream> {
		let mut stream = self
			.transport
			.open_uni(self.transport.session_id())
			.await
			.context("failed to open uni stream")?;

		// TODO set send_order based on header

		header.encode(&mut stream).await?;

		Ok(stream)
	}
}
