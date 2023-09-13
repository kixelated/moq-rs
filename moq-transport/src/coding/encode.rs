use super::BoundsExceeded;

use thiserror::Error;

// I'm too lazy to add these trait bounds to every message type.
// TODO Use trait aliases when they're stable, or add these bounds to every method.
pub trait AsyncWrite: tokio::io::AsyncWrite + Unpin + Send {}
impl AsyncWrite for webtransport_quinn::SendStream {}

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
