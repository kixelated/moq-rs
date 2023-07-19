use std::io::Cursor;
use std::ops::{Deref, DerefMut};

use anyhow::Context;
use bytes::BytesMut;
use moq_transport::{Decode, DecodeError, Encode, Object};
use tokio::io::{AsyncBufReadExt, BufReader};
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

	async fn read(stream: webtransport_quinn::RecvStream) -> anyhow::Result<(Object, RecvStream)> {
		let mut stream = RecvStream::new(stream);

		loop {
			// Read more data into the buffer.
			let data = stream.fill_buf().await?;
			if data.is_empty() {
				anyhow::bail!("stream closed before reading header");
			}

			// Use a cursor to read the buffer and remember how much we read.
			let mut read = Cursor::new(data);

			let header = match Object::decode(&mut read) {
				Ok(header) => header,
				Err(DecodeError::UnexpectedEnd) => continue,
				Err(err) => return Err(err.into()),
			};

			// We parsed a full header, advance the cursor.
			// The borrow checker requires these on separate lines.
			let size = read.position() as usize;
			stream.consume(size);

			return Ok((header, stream));
		}
	}
}

pub type SendStream = webtransport_quinn::SendStream;

// Unfortunately, we need to wrap RecvStream with a buffer since moq-transport::Coding only supports buffered reads.
// TODO support unbuffered reads so we only read the MoQ header and then hand off the stream.
// NOTE: We can't use AsyncRead::chain because we need to get the inner stream for stop.
pub struct RecvStream {
	stream: BufReader<webtransport_quinn::RecvStream>,
}

impl RecvStream {
	fn new(stream: webtransport_quinn::RecvStream) -> Self {
		let stream = BufReader::new(stream);
		Self { stream }
	}

	pub fn stop(self, code: u32) {
		self.stream.into_inner().stop(code).ok();
	}
}

impl Deref for RecvStream {
	type Target = BufReader<webtransport_quinn::RecvStream>;

	fn deref(&self) -> &Self::Target {
		&self.stream
	}
}

impl DerefMut for RecvStream {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.stream
	}
}
