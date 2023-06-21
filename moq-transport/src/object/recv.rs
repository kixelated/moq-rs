use super::Header;

use std::sync::Arc;

// Reduce some typing for implementors.
pub type RecvStream = h3_webtransport::stream::RecvStream<h3_quinn::RecvStream, Bytes>;

// Not clone, so we don't accidentally have two listners.
pub struct Receiver {
	transport: Arc<Transport>,
}

impl Receiver {
	pub async fn recv(&mut self) -> anyhow::Result<(Header, RecvStream)> {
		let (_session_id, mut stream) = self
			.transport
			.accept_uni()
			.await
			.context("failed to accept uni stream")?
			.context("no uni stream")?;

		let header = Header::decode(&mut stream).await?;

		Ok((header, stream))
	}
}
