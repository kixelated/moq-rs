use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::coding::Encode;

use super::EncodeError;

pub struct Writer<S: AsyncWrite + Unpin> {
	stream: S,
	buffer: bytes::BytesMut,
}

impl<S: AsyncWrite + Unpin> Writer<S> {
	pub fn new(stream: S) -> Self {
		Self {
			stream,
			buffer: Default::default(),
		}
	}

	pub async fn encode<T: Encode>(&mut self, msg: &T) -> Result<(), EncodeError> {
		self.buffer.clear();
		msg.encode(&mut self.buffer)?;
		self.stream.write_all(&self.buffer).await?;

		Ok(())
	}

	pub async fn write(&mut self, buf: &[u8]) -> Result<(), EncodeError> {
		self.stream.write_all(buf).await?;
		Ok(())
	}

	pub fn into_inner(self) -> S {
		self.stream
	}
}
