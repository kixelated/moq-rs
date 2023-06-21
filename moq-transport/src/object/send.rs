use super::{Header, SendStream, WebTransportSession};

pub type SendStream = h3_webtransport::stream::SendStream<h3_quinn::SendStream<Bytes>, Bytes>;

#[derive(Clone)]
pub struct Sender {
	transport: Arc<WebTransportSession>,
}

impl Sender {
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
