use thiserror::Error;

use crate::MoqError;

#[derive(Clone, Debug, Error)]
pub enum CacheError {
	/// A clean termination, represented as error code 0.
	/// This error is automatically used when publishers or subscribers are dropped without calling close.
	#[error("closed")]
	Closed,

	/// A SUBSCRIBE_DONE or ANNOUNCE_CANCEL was received.
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

	/// We reported the wrong size for a fragment.
	#[error("wrong size")]
	WrongSize,
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
			Self::WrongSize => 500,
		}
	}
}
