use super::{Header, Receiver, RecvStream, SendStream, Sender};

use anyhow::Context;
use bytes::Bytes;

use crate::coding::{Decode, Encode};

use std::sync::Arc;

// TODO support clients
type WebTransportSession = h3_webtransport::server::WebTransportSession<h3_quinn::Connection, Bytes>;

pub struct Session {
	pub send: Sender,
	pub recv: Receiver,
}

impl Session {
	pub fn new(transport: WebTransportSession) -> Self {
		let shared = Arc::new(transport);

		Self {
			send: Sender::new(shared.clone()),
			recv: Sender::new(shared),
		}
	}

	pub async fn recv(&mut self) -> anyhow::Result<(Header, RecvStream)> {
		self.recv.recv().await
	}

	pub async fn send(&self, header: Header) -> anyhow::Result<SendStream> {
		self.send.send(header).await
	}
}
