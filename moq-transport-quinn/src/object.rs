use anyhow::Context;
use bytes::{Buf, Bytes, BytesMut};
use moq_transport::{Decode, DecodeError, Encode, Object};
use std::{io::Cursor, sync::Arc};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

// TODO support clients
type WebTransportSession = h3_webtransport::server::WebTransportSession<h3_quinn::Connection, Bytes>;

// Reduce some typing
pub type SendStream = h3_webtransport::stream::SendStream<h3_quinn::SendStream<Bytes>, Bytes>;
pub type RecvStream = h3_webtransport::stream::RecvStream<h3_quinn::RecvStream, Bytes>;

pub struct Objects {
	send: SendObjects,
	recv: RecvObjects,
}

impl Objects {
	pub fn new(session: Arc<WebTransportSession>) -> Self {
		let send = SendObjects::new(session.clone());
		let recv = RecvObjects::new(session);
		Self { send, recv }
	}

	pub fn split(self) -> (SendObjects, RecvObjects) {
		(self.send, self.recv)
	}

	pub async fn recv(&mut self) -> anyhow::Result<(Object, RecvStream)> {
		self.recv.recv().await
	}

	pub async fn send(&mut self, header: Object) -> anyhow::Result<SendStream> {
		self.send.send(header).await
	}
}

pub struct SendObjects {
	session: Arc<WebTransportSession>,

	// A reusable buffer for encoding messages.
	buf: BytesMut,
}

impl SendObjects {
	pub fn new(session: Arc<WebTransportSession>) -> Self {
		Self {
			session,
			buf: BytesMut::new(),
		}
	}

	pub async fn send(&mut self, header: Object) -> anyhow::Result<SendStream> {
		self.buf.clear();
		header.encode(&mut self.buf).unwrap();

		let mut stream = self
			.session
			.open_uni(self.session.session_id())
			.await
			.context("failed to open uni stream")?;

		// TODO support select! without making a new stream.
		stream.write_all(&self.buf).await?;

		Ok(stream)
	}
}

impl Clone for SendObjects {
	fn clone(&self) -> Self {
		Self {
			session: self.session.clone(),
			buf: BytesMut::new(),
		}
	}
}

// Not clone, so we don't accidentally have two listners.
pub struct RecvObjects {
	session: Arc<WebTransportSession>,

	// A uni stream that's been accepted but not fully read from yet.
	stream: Option<RecvStream>,

	// Data that we've read but haven't formed a full message yet.
	buf: BytesMut,
}

impl RecvObjects {
	pub fn new(session: Arc<WebTransportSession>) -> Self {
		Self {
			session,
			stream: None,
			buf: BytesMut::new(),
		}
	}

	pub async fn recv(&mut self) -> anyhow::Result<(Object, RecvStream)> {
		// Make sure any state is saved across await boundaries so this works with select!

		let stream = match self.stream.as_mut() {
			Some(stream) => stream,
			None => {
				let (_session_id, stream) = self
					.session
					.accept_uni()
					.await
					.context("failed to accept uni stream")?
					.context("no uni stream")?;

				self.stream.insert(stream)
			}
		};

		loop {
			// Read the contents of the buffer
			let mut peek = Cursor::new(&self.buf);

			match Object::decode(&mut peek) {
				Ok(header) => {
					let stream = self.stream.take().unwrap();
					self.buf.advance(peek.position() as usize);
					return Ok((header, stream));
				}
				Err(DecodeError::UnexpectedEnd) => {
					// The decode failed, so we need to append more data.
					stream.read_buf(&mut self.buf).await?;
				}
				Err(e) => return Err(e.into()),
			}
		}
	}
}
