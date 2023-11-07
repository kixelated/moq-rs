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

	#[error("unsupported versions: client={0:?} server={1:?}")]
	Version(setup::Versions, setup::Versions),

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

	/// Our enforced stream mapping was disrespected.
	#[error("stream mapping conflict")]
	StreamMapping,

	/// The priority was invalid.
	#[error("invalid priority: {0}")]
	InvalidPriority(VarInt),

	/// The size was invalid.
	#[error("invalid size: {0}")]
	InvalidSize(VarInt),

	/// A required extension was not offered.
	#[error("required extension not offered: {0:?}")]
	RequiredExtension(VarInt),

	/// Some VarInt was too large and we were too lazy to handle it
	#[error("varint bounds exceeded")]
	BoundsExceeded(#[from] coding::BoundsExceeded),

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
			Self::StreamMapping => 409,
			Self::Unknown(_) => 500,
			Self::Write(_) => 501,
			Self::Read(_) => 502,
			Self::Session(_) => 503,
			Self::Version(..) => 406,
			Self::Encode(_) => 500,
			Self::Decode(_) => 500,
			Self::InvalidPriority(_) => 400,
			Self::InvalidSize(_) => 400,
			Self::RequiredExtension(_) => 426,
			Self::BoundsExceeded(_) => 500,
		}
	}

	/// A reason that is sent over the wire.
	fn reason(&self) -> String {
		match self {
			Self::Cache(err) => err.reason(),
			Self::RoleViolation(kind) => format!("role violation for message type {:?}", kind),
			Self::RoleIncompatible(client, server) => {
				format!(
					"role incompatible: client wanted {:?} but server wanted {:?}",
					client, server
				)
			}
			Self::Read(err) => format!("read error: {}", err),
			Self::Write(err) => format!("write error: {}", err),
			Self::Session(err) => format!("session error: {}", err),
			Self::Unknown(err) => format!("unknown error: {}", err),
			Self::Version(client, server) => format!("unsupported versions: client={:?} server={:?}", client, server),
			Self::Encode(err) => format!("encode error: {}", err),
			Self::Decode(err) => format!("decode error: {}", err),
			Self::StreamMapping => "streaming mapping conflict".to_owned(),
			Self::InvalidPriority(priority) => format!("invalid priority: {}", priority),
			Self::InvalidSize(size) => format!("invalid size: {}", size),
			Self::RequiredExtension(id) => format!("required extension was missing: {:?}", id),
			Self::BoundsExceeded(_) => "varint bounds exceeded".to_string(),
		}
	}
}
