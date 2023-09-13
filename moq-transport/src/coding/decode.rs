use super::{BoundsExceeded, VarInt};
use std::str;

use thiserror::Error;

// I'm too lazy to add these trait bounds to every message type.
// TODO Use trait aliases when they're stable, or add these bounds to every method.
pub trait AsyncRead: tokio::io::AsyncRead + Unpin + Send {}
impl AsyncRead for webtransport_quinn::RecvStream {}

/// A decode error.
#[derive(Error, Debug)]
pub enum DecodeError {
	#[error("unexpected end of buffer")]
	UnexpectedEnd,

	#[error("invalid string")]
	InvalidString(#[from] str::Utf8Error),

	#[error("invalid type: {0:?}")]
	InvalidType(VarInt),

	#[error("varint bounds exceeded")]
	BoundsExceeded(#[from] BoundsExceeded),

	#[error("io error: {0}")]
	IoError(#[from] std::io::Error),
}
