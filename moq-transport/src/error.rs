use thiserror::Error;

use crate::VarInt;

/// A MoQTransport error with an associated error code.
#[derive(Copy, Clone, Debug, Error)]
pub enum Error {
	#[error("closed")]
	Closed,

	#[error("reset code={0:?}")]
	Reset(u32),

	#[error("stop code={0:?}")]
	Stop(u32),

	#[error("not found")]
	NotFound,

	#[error("duplicate")]
	Duplicate,

	/// The remote sent a message that violates the negotiated role. ex. sending SUBSCRIBE to a subscriber.
	#[error("role violation: msg={0}")]
	Role(VarInt),

	#[error("failed to read from stream")]
	Read,

	#[error("failed to write to stream")]
	Write,

	// TODO classify these errors
	#[error("unknown error")]
	Unknown,
}

impl Error {
	/// An integer code that is sent over the wire.
	pub fn code(&self) -> u32 {
		match self {
			Self::Closed => 0,
			Self::Reset(code) => *code,
			Self::Stop(code) => *code,
			Self::NotFound => 404,
			Self::Role(_) => 405,
			Self::Duplicate => 409,
			Self::Unknown => 500,
			Self::Write => 501,
			Self::Read => 502,
		}
	}

	/// A reason that is sent over the wire.
	pub fn reason(&self) -> &str {
		match self {
			Self::Closed => "closed",
			Self::Reset(_) => "reset",
			Self::Stop(_) => "stop",
			Self::NotFound => "not found",
			Self::Duplicate => "duplicate",
			Self::Role(_msg) => "role violation",
			Self::Unknown => "unknown",
			Self::Read => "read error",
			Self::Write => "write error",
		}
	}
}
