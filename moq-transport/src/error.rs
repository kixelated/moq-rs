use crate::{coding, setup};

#[derive(thiserror::Error, Debug, Clone)]
pub enum SessionError {
	#[error("webtransport error: {0}")]
	Session(#[from] webtransport_quinn::SessionError),

	#[error("encode error: {0}")]
	Encode(#[from] coding::EncodeError),

	#[error("decode error: {0}")]
	Decode(#[from] coding::DecodeError),

	// TODO move to a ConnectError
	#[error("unsupported versions: client={0:?} server={1:?}")]
	Version(setup::Versions, setup::Versions),

	// TODO move to a ConnectError
	#[error("incompatible roles: client={0:?} server={1:?}")]
	RoleIncompatible(setup::Role, setup::Role),

	/// An error occured while reading from the QUIC stream.
	#[error("failed to read from stream: {0}")]
	Read(#[from] webtransport_quinn::ReadError),

	/// An error occured while writing to the QUIC stream.
	#[error("failed to write to stream: {0}")]
	Write(#[from] webtransport_quinn::WriteError),

	/// The role negiotiated in the handshake was violated. For example, a publisher sent a SUBSCRIBE, or a subscriber sent an OBJECT.
	#[error("role violation")]
	RoleViolation,

	/// Some VarInt was too large and we were too lazy to handle it
	#[error("varint bounds exceeded")]
	BoundsExceeded(#[from] coding::BoundsExceeded),

	/// A duplicate ID was used
	#[error("duplicate")]
	Duplicate,

	#[error("internal error")]
	Internal,
}

impl SessionError {
	/// An integer code that is sent over the wire.
	pub fn code(&self) -> u64 {
		match self {
			Self::RoleIncompatible(..) => 406,
			Self::RoleViolation => 405,
			Self::Write(_) => 501,
			Self::Read(_) => 400,
			Self::Session(_) => 503,
			Self::Version(..) => 406,
			Self::Decode(_) => 400,
			Self::Encode(_) => 500,
			Self::BoundsExceeded(_) => 500,
			Self::Duplicate => 409,
			Self::Internal => 500,
		}
	}
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum SubscribeError {
	/// Unsubscribe
	#[error("unsubscribe")]
	Cancel,

	/// A SUBSCRIBE_ERROR
	#[error("subscribe error, code={0}")]
	Error(u64),

	/// A SUBSCRIBE_DONE
	#[error("subscribe done, code={0}")]
	Done(u64),

	/// The subscribe was not found, possibly because it was already closed
	#[error("subscribe not found")]
	NotFound,

	/// We received a duplicate message, like two SUBSCRIBES or SUBSCRIBE_OKs
	#[error("duplicate message")]
	Duplicate,

	/// The session encountered an error
	#[error("session error: {0}")]
	Session(#[from] SessionError),
}

impl SubscribeError {
	pub fn code(&self) -> u64 {
		match self {
			Self::Cancel => 0,
			Self::Error(code) => *code,
			Self::Done(code) => *code,
			Self::NotFound => 404,
			Self::Duplicate => 409,
			Self::Session(_) => 500,
		}
	}
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum AnnounceError {
	/// An UNANNOUNCE
	#[error("unannounce")]
	Done,

	/// An ANNOUNCE_ERROR
	#[error("announce error, code={0}")]
	Error(u64),

	/// An ANNOUNCE_CANCEL
	// TODO this should have an error code
	#[error("announce cancel")]
	Cancel,

	/// The announce was not found, possibly because it was already closed
	#[error("announce not found")]
	NotFound,

	/// We received a duplicate message, like two ANNOUNCEs or ANNOUNCE_OKs
	#[error("duplicate message")]
	Duplicate,

	/// The session encountered an error
	#[error("session error: {0}")]
	Session(#[from] SessionError),
}

impl AnnounceError {
	pub fn code(&self) -> u64 {
		match self {
			Self::Done => 0,
			Self::Error(code) => *code,
			Self::Cancel => 1,
			Self::NotFound => 404,
			Self::Duplicate => 409,
			Self::Session(_) => 500,
		}
	}
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum ReadError {
	#[error("subscribe error: {0}")]
	Subscribe(#[from] SubscribeError),

	#[error("decode error: {0}")]
	Decode(#[from] coding::DecodeError),

	/// An error occured while reading from the QUIC stream.
	#[error("stream error: {0}")]
	Stream(#[from] webtransport_quinn::ReadError),

	#[error("short read")]
	Short,
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum WriteError {
	#[error("subscribe error: {0}")]
	Subscribe(#[from] SubscribeError),

	#[error("encode error: {0}")]
	Encode(#[from] coding::EncodeError),

	/// An error occured while writing to the QUIC stream.
	#[error("stream error: {0}")]
	Stream(#[from] webtransport_quinn::WriteError),

	/// The caller wrote the wrong payload size
	#[error("wrong payload size")]
	WrongSize,

	/// The caller wrote objects in the wrong order
	#[error("wrong object order")]
	WrongOrder,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum CacheError {
	#[error("done")]
	Done,

	#[error("wrong size")]
	WrongSize,

	#[error("closed: code={0}")]
	Closed(u64),

	#[error("duplicate")]
	Duplicate,

	#[error("multiple stream modes")]
	Mode,
}

/*
/// An error that causes the session to close.
#[derive(thiserror::Error, Debug)]
pub enum SessionError {
	// Official error codes
	#[error("closed")]
	Closed,

	#[error("internal error")]
	Internal,

	#[error("unauthorized")]
	Unauthorized,

	#[error("protocol violation")]
	ProtocolViolation,

	#[error("duplicate track alias")]
	DuplicateTrackAlias,

	#[error("parameter length mismatch")]
	ParameterLengthMismatch,

	#[error("goaway timeout")]
	GoawayTimeout,

	#[error("unknown error: code={0}")]
	Unknown(u64),
	// Unofficial error codes
}

impl MoqError for SessionError {
	/// An integer code that is sent over the wire.
	fn code(&self) -> u64 {
		match self {
			// Official error codes
			Self::Closed => 0x0,
			Self::Internal => 0x1,
			Self::Unauthorized => 0x2,
			Self::ProtocolViolation => 0x3,
			Self::DuplicateTrackAlias => 0x4,
			Self::ParameterLengthMismatch => 0x5,
			Self::GoawayTimeout => 0x10,
			Self::Unknown(code) => *code,
			// Unofficial error codes
		})
	}
}

/// An error that causes the subscribe to be rejected immediately.
#[derive(thiserror::Error, Debug)]
pub enum SubscribeError {
	// Official error codes
	#[error("internal error")]
	Internal,

	#[error("invalid range")]
	InvalidRange,

	#[error("retry alias")]
	RetryAlias,

	#[error("unknown error: code={0}")]
	Unknown(u64),
	// Unofficial error codes
}

impl MoqError for SubscribeError {
	/// An integer code that is sent over the wire.
	fn code(&self) -> u64 {
		match self {
			// Official error codes
			Self::Internal => 0x0,
			Self::InvalidRange => 0x1,
			Self::RetryAlias => 0x2,
			Self::Unknown(code) => *code,
			// Unofficial error codes
		}
	}
}

/// An error that causes the subscribe to be terminated.
#[derive(thiserror::Error, Debug)]
pub enum SubscribeDone {
	// Official error codes
	#[error("unsubscribed")]
	Unsubscribed,

	#[error("internal error")]
	Internal,

	// TODO This should be in SubscribeError
	#[error("unauthorized")]
	Unauthorized,

	#[error("track ended")]
	TrackEnded,

	// TODO What the heck is this?
	#[error("subscription ended")]
	SubscriptionEnded,

	#[error("going away")]
	GoingAway,

	#[error("expired")]
	Expired,

	#[error("unknown error: code={0}")]
	Unknown(u64),
}

impl From<u64> for SubscribeDone {
	fn from(code: u64) -> Self {
		match code.into_inner() {
			0x0 => Self::Unsubscribed,
			0x1 => Self::Internal,
			0x2 => Self::Unauthorized,
			0x3 => Self::TrackEnded,
			0x4 => Self::SubscriptionEnded,
			0x5 => Self::GoingAway,
			0x6 => Self::Expired,
			_ => Self::Unknown(code),
		}
	}
}

impl MoqError for SubscribeDone {
	/// An integer code that is sent over the wire.
	fn code(&self) -> u64 {
		match self {
			// Official error codes
			Self::Unsubscribed => 0x0,
			Self::Internal => 0x1,
			Self::Unauthorized => 0x2,
			Self::TrackEnded => 0x3,
			Self::SubscriptionEnded => 0x4,
			Self::GoingAway => 0x5,
			Self::Expired => 0x6,
			Self::Unknown(code) => *code,
			// Unofficial error codes
		}
	}
}
*/
