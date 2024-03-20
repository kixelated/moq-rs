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
