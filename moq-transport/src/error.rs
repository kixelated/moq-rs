use thiserror::Error;

use crate::VarInt;

/// A MoQTransport error with an associated error code.
#[derive(Clone, Debug, Error)]
pub enum Error {
	/// A clean termination, represented as error code 0.
	/// This error is automatically used when publishers or subscribers are dropped without calling close.
	#[error("closed")]
	Closed,

	/// A session error occured.
	#[error("session error: {0}")]
	Session(#[from] webtransport_quinn::SessionError),

	/// An ANNOUNCE_RESET or SUBSCRIBE_RESET was sent by the publisher.
	#[error("reset code={0:?}")]
	Reset(u32),

	/// An ANNOUNCE_STOP or SUBSCRIBE_STOP was sent by the subscriber.
	#[error("stop")]
	Stop,

	/// The requested resource was not found.
	#[error("not found")]
	NotFound,

	/// A resource already exists with that ID.
	#[error("duplicate")]
	Duplicate,

	/// The role negiotiated in the handshake was violated. For example, a publisher sent a SUBSCRIBE, or a subscriber sent an OBJECT.
	#[error("role violation: msg={0}")]
	Role(VarInt),

	/// An error occured while reading from the QUIC stream.
	#[error("failed to read from stream: {0}")]
	Read(#[from] webtransport_quinn::ReadError),

	/// An error occured while writing to the QUIC stream.
	#[error("failed to write to stream: {0}")]
	Write(#[from] webtransport_quinn::WriteError),

	/// An unclassified error because I'm lazy. TODO classify these errors
	#[error("unknown error: {0}")]
	Unknown(String),
}

impl Error {
	/// An integer code that is sent over the wire.
	pub fn code(&self) -> u32 {
		match self {
			Self::Closed => 0,
			Self::Reset(code) => *code,
			Self::Stop => 206,
			Self::NotFound => 404,
			Self::Role(_) => 405,
			Self::Duplicate => 409,
			Self::Unknown(_) => 500,
			Self::Write(_) => 501,
			Self::Read(_) => 502,
			Self::Session(_) => 503,
		}
	}

	/// A reason that is sent over the wire.
	pub fn reason(&self) -> &str {
		match self {
			Self::Closed => "closed",
			Self::Reset(_) => "reset",
			Self::Stop => "stop",
			Self::NotFound => "not found",
			Self::Duplicate => "duplicate",
			Self::Role(_) => "role violation",
			Self::Read(_) => "read error",
			Self::Write(_) => "write error",
			Self::Session(_) => "session error",
			Self::Unknown(_) => "unknown",
		}
	}
}
