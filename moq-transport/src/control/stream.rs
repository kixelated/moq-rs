use crate::coding::{Decode, Encode};
use crate::control::Message;

use bytes::Bytes;

use h3::quic::BidiStream;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Stream {
	sender: SendStream,
	recver: RecvStream,
}

impl Stream {
	pub(crate) fn new(stream: h3_webtransport::stream::BidiStream<h3_quinn::BidiStream<Bytes>, Bytes>) -> Self {
		let (sender, recver) = stream.split();
		let sender = SendStream { stream: sender };
		let recver = RecvStream { stream: recver };

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
	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let msg = msg.into();
		log::info!("sending message: {:?}", msg);
		msg.encode(&mut self.stream).await
	}

	// Helper that lets multiple threads send control messages.
	pub fn share(self) -> SendShared {
		SendShared {
			stream: Arc::new(Mutex::new(self)),
		}
	}
}

// Helper that allows multiple threads to send control messages.
#[derive(Clone)]
pub struct SendShared {
	stream: Arc<Mutex<SendStream>>,
}

impl SendShared {
	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let mut stream = self.stream.lock().await;
		stream.send(msg).await
	}
}

pub struct RecvStream {
	stream: h3_webtransport::stream::RecvStream<h3_quinn::RecvStream, Bytes>,
}

impl RecvStream {
	pub async fn recv(&mut self) -> anyhow::Result<Message> {
		let msg = Message::decode(&mut self.stream).await?;
		log::info!("received message: {:?}", msg);
		Ok(msg)
	}
}
