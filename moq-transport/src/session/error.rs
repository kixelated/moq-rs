use crate::{cache, coding, setup, MoqError, VarInt};

#[derive(thiserror::Error, Debug)]
pub enum SessionError {
	#[error("webtransport error: {0}")]
	Session(#[from] webtransport_quinn::SessionError),

	#[error("cache error: {0}")]
	Cache(#[from] cache::CacheError),

	#[error("encode error: {0}")]
	Encode(#[from] coding::EncodeError),

	#[error("decode error: {0}")]
	Decode(#[from] coding::DecodeError),

	#[error("unsupported version: {0:?}")]
	Version(Option<setup::Version>),

	#[error("incompatible roles: client={0:?} server={1:?}")]
	RoleIncompatible(setup::Role, setup::Role),

	/// An error occured while reading from the QUIC stream.
	#[error("failed to read from stream: {0}")]
	Read(#[from] webtransport_quinn::ReadError),

	/// An error occured while writing to the QUIC stream.
	#[error("failed to write to stream: {0}")]
	Write(#[from] webtransport_quinn::WriteError),

	/// The role negiotiated in the handshake was violated. For example, a publisher sent a SUBSCRIBE, or a subscriber sent an OBJECT.
	#[error("role violation: msg={0}")]
	RoleViolation(VarInt),

	/// An unclassified error because I'm lazy. TODO classify these errors
	#[error("unknown error: {0}")]
	Unknown(String),
}

impl MoqError for SessionError {
	/// An integer code that is sent over the wire.
	fn code(&self) -> u32 {
		match self {
			Self::Cache(err) => err.code(),
			Self::RoleIncompatible(..) => 406,
			Self::RoleViolation(..) => 405,
			Self::Unknown(_) => 500,
			Self::Write(_) => 501,
			Self::Read(_) => 502,
			Self::Session(_) => 503,
			Self::Version(_) => 406,
			Self::Encode(_) => 500,
			Self::Decode(_) => 500,
		}
	}

	/// A reason that is sent over the wire.
	fn reason(&self) -> &str {
		match self {
			Self::Cache(err) => err.reason(),
			Self::RoleViolation(_) => "role violation",
			Self::RoleIncompatible(..) => "role incompatible",
			Self::Read(_) => "read error",
			Self::Write(_) => "write error",
			Self::Session(_) => "session error",
			Self::Unknown(_) => "unknown",
			Self::Version(_) => "unsupported version",
			Self::Encode(_) => "encode error",
			Self::Decode(_) => "decode error",
		}
	}
}
