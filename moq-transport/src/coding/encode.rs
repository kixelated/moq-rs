use super::BoundsExceeded;

use thiserror::Error;

// I'm too lazy to add these trait bounds to every message type.
// TODO Use trait aliases when they're stable, or add these bounds to every method.
pub trait AsyncWrite: tokio::io::AsyncWrite + Unpin + Send {}
impl AsyncWrite for webtransport_quinn::SendStream {}
impl AsyncWrite for Vec<u8> {}

#[async_trait::async_trait]
pub trait Encode: Sized {
	async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError>;
}

/// An encode error.
#[derive(Error, Debug)]
pub enum EncodeError {
	#[error("varint too large")]
	BoundsExceeded(#[from] BoundsExceeded),

	#[error("invalid value")]
	InvalidValue,

	#[error("i/o error: {0}")]
	IoError(#[from] std::io::Error),
}
