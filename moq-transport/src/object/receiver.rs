use std::io::Cursor;
use std::task::{self, Poll};

use crate::coding::{Decode, DecodeError};
use crate::object::Header;

use anyhow::Context;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio::task::JoinSet;

use webtransport_generic::RecvStream as GenericRecvStream;
use webtransport_generic::{AsyncRecvStream, AsyncSession};

pub struct Receiver<S>
where
	S: AsyncSession,
{
	session: S,

	// Streams that we've accepted but haven't read the header from yet.
	streams: JoinSet<anyhow::Result<(Header, RecvStream<S::RecvStream>)>>,
}

impl<S> Receiver<S>
where
	S: AsyncSession,
	S::RecvStream: AsyncRecvStream,
{
	pub fn new(session: S) -> Self {
		Self {
			session,
			streams: JoinSet::new(),
		}
	}

	pub async fn recv(&mut self) -> anyhow::Result<(Header, RecvStream<S::RecvStream>)> {
		loop {
			tokio::select! {
				res = self.session.accept_uni() => {
					let stream = res.context("failed to accept stream")?;
					self.streams.spawn(async move { Self::read(stream).await });
				},
				res = self.streams.join_next(), if !self.streams.is_empty() => {
					return res.unwrap().context("failed to run join set")?;
				}
			}
		}
	}

	async fn read(mut stream: S::RecvStream) -> anyhow::Result<(Header, RecvStream<S::RecvStream>)> {
		let mut buf = BytesMut::new();

		loop {
			// Read more data into the buffer.
			stream.recv(&mut buf).await?;

			// Use a cursor to read the buffer and remember how much we read.
			let mut read = Cursor::new(&mut buf);

			let header = match Header::decode(&mut read) {
				Ok(header) => header,
				Err(DecodeError::UnexpectedEnd) => continue,
				Err(err) => return Err(err.into()),
			};

			// We parsed a full header, advance the buffer.
			let size = read.position() as usize;
			buf.advance(size);
			let buf = buf.freeze();

			// log::info!("received stream: {:?}", header);

			let stream = RecvStream::new(buf, stream);

			return Ok((header, stream));
		}
	}
}

// Unfortunately, we need to wrap RecvStream with a buffer since moq-transport::Coding only supports buffered reads.
// We first serve any data in the buffer, then we poll the stream.
// TODO fix this so we don't need the wrapper.
pub struct RecvStream<R>
where
	R: GenericRecvStream,
{
	buf: Bytes,
	stream: R,
}

impl<R> RecvStream<R>
where
	R: GenericRecvStream,
{
	pub(crate) fn new(buf: Bytes, stream: R) -> Self {
		Self { buf, stream }
	}

	pub fn stop(&mut self, code: u32) {
		self.stream.stop(code)
	}
}

impl<R> GenericRecvStream for RecvStream<R>
where
	R: GenericRecvStream,
{
	type Error = R::Error;

	fn poll_recv<B: BufMut>(
		&mut self,
		cx: &mut task::Context<'_>,
		buf: &mut B,
	) -> Poll<Result<Option<usize>, Self::Error>> {
		if !self.buf.is_empty() {
			let size = self.buf.len();
			buf.put(&mut self.buf);
			let size = size - self.buf.len();
			Poll::Ready(Ok(Some(size)))
		} else {
			self.stream.poll_recv(cx, buf)
		}
	}

	fn stop(&mut self, error_code: u32) {
		self.stream.stop(error_code)
	}
}
