use crate::coding::{Decode, DecodeError};
use crate::message::Message;

use bytes::{Buf, BytesMut};

use std::io::Cursor;

use webtransport_generic::AsyncRecvStream;

pub struct Receiver<R>
where
	R: AsyncRecvStream, // TODO take RecvStream instead
{
	stream: R,
	buf: BytesMut, // data we've read but haven't fully decoded yet
}

impl<R> Receiver<R>
where
	R: AsyncRecvStream,
{
	pub fn new(stream: R) -> Self {
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
					return Ok(msg);
				}
				Err(DecodeError::UnexpectedEnd) => {
					// The decode failed, so we need to append more data.
					self.stream.recv(&mut self.buf).await?;
				}
				Err(e) => return Err(e.into()),
			}
		}
	}
}
