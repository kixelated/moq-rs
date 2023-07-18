use anyhow::Context;
use moq_transport::{Decode, DecodeError, Encode, Message};

use bytes::{Buf, BufMut, BytesMut};

use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::Mutex;

use webtransport_quinn::{RecvStream, SendStream};

pub struct SendControl {
	stream: SendStream,
	buf: BytesMut, // reuse a buffer to encode messages.
}

impl SendControl {
	pub fn new(stream: SendStream) -> Self {
		Self {
			buf: BytesMut::new(),
			stream,
		}
	}

	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let msg = msg.into();
		log::info!("sending message: {:?}", msg);

		self.buf.clear();
		msg.encode(&mut self.buf)?;

		// TODO make this work with select!
		self.stream.write_all(&self.buf).await?;

		Ok(())
	}

	// Helper that lets multiple threads send control messages.
	pub fn share(self) -> ControlShared {
		ControlShared {
			stream: Arc::new(Mutex::new(self)),
		}
	}
}

// Helper that allows multiple threads to send control messages.
// There's no equivalent for receiving since only one thread should be receiving at a time.
#[derive(Clone)]
pub struct ControlShared {
	stream: Arc<Mutex<SendControl>>,
}

impl ControlShared {
	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let mut stream = self.stream.lock().await;
		stream.send(msg).await
	}
}

pub struct RecvControl {
	stream: RecvStream,
	buf: BytesMut, // data we've read but haven't fully decoded yet
}

impl RecvControl {
	pub fn new(stream: RecvStream) -> Self {
		Self {
			buf: BytesMut::new(),
			stream,
		}
	}

	// Read the next full message from the stream.
	pub async fn recv(&mut self) -> anyhow::Result<Message> {
		loop {
			// Read the contents of the buffer
			let mut peek = Cursor::new(&self.buf);

			match Message::decode(&mut peek) {
				Ok(msg) => {
					// We've successfully decoded a message, so we can advance the buffer.
					self.buf.advance(peek.position() as usize);

					log::info!("received message: {:?}", msg);
					return Ok(msg);
				}
				Err(DecodeError::UnexpectedEnd) => {
					// The decode failed, so we need to append more data.
					let chunk = self.stream.read_chunk(1024, true).await?.context("stream closed")?;
					self.buf.put(chunk.bytes);
				}
				Err(e) => return Err(e.into()),
			}
		}
	}
}
