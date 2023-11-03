use thiserror::Error;

use crate::MoqError;

#[derive(Clone, Debug, Error)]
pub enum CacheError {
	/// A clean termination, represented as error code 0.
	/// This error is automatically used when publishers or subscribers are dropped without calling close.
	#[error("closed")]
	Closed,

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
}

impl MoqError for CacheError {
	/// An integer code that is sent over the wire.
	fn code(&self) -> u32 {
		match self {
			Self::Closed => 0,
			Self::Reset(code) => *code,
			Self::Stop => 206,
			Self::NotFound => 404,
			Self::Duplicate => 409,
		}
	}

	/// A reason that is sent over the wire.
	fn reason(&self) -> String {
		match self {
			Self::Closed => "closed".to_owned(),
			Self::Reset(code) => format!("reset code: {}", code),
			Self::Stop => "stop".to_owned(),
			Self::NotFound => "not found".to_owned(),
			Self::Duplicate => "duplicate".to_owned(),
		}
	}
}
