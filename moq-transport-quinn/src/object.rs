use anyhow::Context;
use bytes::BytesMut;
use moq_transport::{Decode, Encode, Object};
use tokio::task::JoinSet;

use super::{RecvStream, SendStream};
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
