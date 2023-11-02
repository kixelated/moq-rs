use super::{BoundsExceeded, VarInt};
use std::{io, str};

use thiserror::Error;

// I'm too lazy to add these trait bounds to every message type.
// TODO Use trait aliases when they're stable, or add these bounds to every method.
pub trait AsyncRead: tokio::io::AsyncRead + Unpin + Send {}
impl AsyncRead for webtransport_quinn::RecvStream {}
impl<T> AsyncRead for tokio::io::Take<&mut T> where T: AsyncRead {}
impl<T: AsRef<[u8]> + Unpin + Send> AsyncRead for io::Cursor<T> {}

#[async_trait::async_trait]
pub trait Decode: Sized {
	async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError>;
}

/// A decode error.
#[derive(Error, Debug)]
pub enum DecodeError {
	#[error("unexpected end of buffer")]
	UnexpectedEnd,

	#[error("invalid string")]
	InvalidString(#[from] str::Utf8Error),

	#[error("invalid message: {0:?}")]
	InvalidMessage(VarInt),

	#[error("invalid role: {0:?}")]
	InvalidRole(VarInt),

	#[error("invalid subscribe location")]
	InvalidSubscribeLocation,

	#[error("varint bounds exceeded")]
	BoundsExceeded(#[from] BoundsExceeded),

	// TODO move these to ParamError
	#[error("duplicate parameter")]
	DupliateParameter,

	#[error("missing parameter")]
	MissingParameter,

	#[error("invalid parameter")]
	InvalidParameter,

	#[error("io error: {0}")]
	IoError(#[from] std::io::Error),

	// Used to signal that the stream has ended.
	#[error("no more messages")]
	Final,
}
