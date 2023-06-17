use crate::coding::{Decode, Encode};
use crate::control::Message;

use bytes::Bytes;

use h3::quic::BidiStream;

pub struct Stream {
	sender: SendStream,
	recver: RecvStream,
}

impl Stream {
	pub(crate) fn new(stream: h3_webtransport::stream::BidiStream<h3_quinn::BidiStream<Bytes>, Bytes>) -> Self {
		let (sender, recver) = stream.split();
		let sender = SendStream::new(sender);
		let recver = RecvStream::new(recver);

		Self { sender, recver }
	}

	pub fn split(self) -> (SendStream, RecvStream) {
		(self.sender, self.recver)
	}

	pub async fn send(&mut self, msg: Message) -> anyhow::Result<()> {
		self.sender.send(msg).await
	}

	pub async fn recv(&mut self) -> anyhow::Result<Message> {
		self.recver.recv().await
	}
}

pub struct SendStream {
	stream: h3_webtransport::stream::SendStream<h3_quinn::SendStream<Bytes>, Bytes>,
}

impl SendStream {
	pub(crate) fn new(stream: h3_webtransport::stream::SendStream<h3_quinn::SendStream<Bytes>, Bytes>) -> Self {
		Self { stream }
	}

	pub async fn send(&mut self, msg: Message) -> anyhow::Result<()> {
		msg.encode(&mut self.stream).await
	}
}

pub struct RecvStream {
	stream: h3_webtransport::stream::RecvStream<h3_quinn::RecvStream, Bytes>,
}

impl RecvStream {
	pub(crate) fn new(stream: h3_webtransport::stream::RecvStream<h3_quinn::RecvStream, Bytes>) -> Self {
		Self { stream }
	}

	pub async fn recv(&mut self) -> anyhow::Result<Message> {
		Message::decode(&mut self.stream).await
	}
}
