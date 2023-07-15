use webtransport_generic::{RecvStream, SendStream};
use crate::{Decode, DecodeError, Encode, Message};
use crate::network::stream::{recv, send};

use bytes::{Buf, BytesMut};

use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::Mutex;

use anyhow::Context;


pub struct Control<S: SendStream, R: RecvStream> {
	sender: ControlSend<S>,
	recver: ControlRecv<R>,
}

impl<S: SendStream, R: RecvStream> Control<S, R>{
	pub(crate) fn new(sender: Box<S>, recver: Box<R>) -> Self {
		let sender = ControlSend::new(sender);
		let recver = ControlRecv::new(recver);

		Self { sender, recver }
	}

	pub fn split(self) -> (ControlSend<S>, ControlRecv<R>) {
		(self.sender, self.recver)
	}

	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		self.sender.send(msg)
			.await
			.map_err(|e| anyhow::anyhow!("{:?}", e))
			.context("error sending control message")
	}

	pub async fn recv(&mut self) -> anyhow::Result<Message> {
		self.recver.recv().await
	}
}

pub struct ControlSend<S: SendStream> {
	stream: Box<S>,
	buf: BytesMut, // reuse a buffer to encode messages.
}

impl<S: SendStream> ControlSend<S> {
	pub fn new(inner: Box<S>) -> Self {
		Self {
			buf: BytesMut::new(),
			stream: inner,
		}
	}

	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let msg = msg.into();
		log::info!("sending message: {:?}", msg);

		self.buf.clear();
		msg.encode(&mut self.buf)?;

		// TODO make this work with select!
		send(self.stream.as_mut(), &mut self.buf)
		.await
		.map_err(|e| anyhow::anyhow!("{:?}", e.into()))
		.context("error sending control message")?;
		Ok(())
	}

	// Helper that lets multiple threads send control messages.
	pub fn share(self) -> ControlShared<S> {
		ControlShared {
			stream: Arc::new(Mutex::new(self)),
		}
	}
}

// Helper that allows multiple threads to send control messages.
// There's no equivalent for receiving since only one thread should be receiving at a time.
#[derive(Clone)]
pub struct ControlShared<S: SendStream> {
	stream: Arc<Mutex<ControlSend<S>>>,
}

impl<S: SendStream> ControlShared<S> {
	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let mut stream = self.stream.lock().await;
		stream.send(msg).await
	}
}

pub struct ControlRecv<R: RecvStream> {
	stream: Box<R>,
	buf: BytesMut, // data we've read but haven't fully decoded yet
}

impl<R: RecvStream> ControlRecv<R> {
	pub fn new(inner: Box<R>) -> Self {
		Self {
			buf: BytesMut::new(),
			stream: inner,
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
					recv(self.stream.as_mut(), &mut self.buf)
						.await
						.map_err(|e| anyhow::anyhow!("{:?}", e.into()))
						.context("error receiving control message")?;
				}
				Err(e) => return Err(e.into()),
			}
		}
	}
}
