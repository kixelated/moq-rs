use anyhow::Context;
use bytes::{BytesMut};
use moq_transport::{Decode, Encode, Object};
use tokio::task::JoinSet;

use webtransport_quinn::Session;

pub struct SendObjects {
	session: Session,

	// A reusable buffer for encoding messages.
	buf: BytesMut,
}

impl SendObjects {
	pub fn new(session: Session) -> Self {
		Self {
			session,
			buf: BytesMut::new(),
		}
	}

	pub async fn send(&mut self, header: Object) -> anyhow::Result<SendStream> {
		self.buf.clear();
		header.encode(&mut self.buf).unwrap();

		let mut stream = self.session.open_uni().await.context("failed to open uni stream")?;

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
	session: Session,

	// Streams that we've accepted but haven't read the header from yet.
	streams: JoinSet<anyhow::Result<(Object, RecvStream)>>,
}

impl RecvObjects {
	pub fn new(session: Session) -> Self {
		Self {
			session,
			streams: JoinSet::new(),
		}
	}

	pub async fn recv(&mut self) -> anyhow::Result<(Object, RecvStream)> {
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

	async fn read(mut stream: RecvStream) -> anyhow::Result<(Object, RecvStream)> {
		let mut chunk = stream.read_chunk(usize::MAX, true).await?.context("no more data")?;

		// TODO buffer more on UnexpectedEnd
		// TODO read ONLY the header.
		let header = Object::decode(&mut chunk.bytes)?;

		Ok((header, stream))
	}
}

pub type SendStream = webtransport_quinn::SendStream;
pub type RecvStream = webtransport_quinn::RecvStream;

/*
// Unfortunately we need a wrapper for reading, because the moq-transport::Coding API means we read too much of the stream.
pub struct RecvStream {
	stream: webtransport_quinn::RecvStream,
	buffer: Bytes,
}

impl RecvStream {
	pub fn new(stream: webtransport_quinn::RecvStream, buffer: Bytes) -> Self {
		Self { stream, buffer }
	}

	pub async fn read(&mut self, dst: &mut [u8]) -> Result<Option<usize>, webtransport_quinn::ReadError> {
		if !self.buffer.is_empty() {
			let size = std::cmp::min(dst.len(), self.buffer.len());
			self.buffer.copy_to_slice(&mut dst[..size]);
			Ok(Some(size))
		} else {
			self.stream.read(dst).await
		}
	}

	pub async fn read_exact(&mut self, mut buf: &mut [u8]) -> Result<(), webtransport_quinn::ReadExactError> {
		if self.buffer.is_empty() {
			let size = std::cmp::min(buf.len(), self.buffer.len());
			self.buffer.copy_to_slice(&mut buf[..size]);
			buf = &mut buf[size..];
		}

		self.stream.read_exact(&mut buf).await
	}

	pub async fn read_chunk(
		&mut self,
		max_length: usize,
		ordered: bool,
	) -> Result<Option<quinn::Chunk>, webtransport_quinn::ReadError> {
		if self.buffer.is_empty() {
		}
	}

	pub async fn read_chunks(&mut self, bufs: &mut [Bytes]) -> Result<Option<usize>, webtransport_quinn::ReadError> {}

	pub async fn read_to_end(&mut self, size_limit: usize) -> Result<Vec<u8>, webtransport_quinn::ReadToEndError> {}

	pub fn stop(&mut self, error_code: u32) -> Result<(), webtransport_quinn::UnknownStream> {
		self.stream.stop(error_code)
	}
}
*/
