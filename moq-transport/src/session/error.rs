use std::{io, sync};

use crate::{coding, serve, setup};

#[derive(thiserror::Error, Debug, Clone)]
pub enum SessionError {
	#[error("webtransport error: {0}")]
	WebTransport(sync::Arc<dyn webtransport_generic::SessionError>),

	// This needs an Arc because it's not Clone.
	#[error("io error: {0}")]
	Io(sync::Arc<io::Error>),

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

	#[error("cache error: {0}")]
	Cache(#[from] serve::ServeError),

	#[error("wrong size")]
	WrongSize,
}

/*
impl<T: webtransport_generic::SessionError> From<T> for SessionError {
	fn from(err: T) -> Self {
		Self::WebTransport(sync::Arc::new(err))
	}
}
*/

impl From<io::Error> for SessionError {
	fn from(err: io::Error) -> Self {
		Self::Io(sync::Arc::new(err))
	}
}

impl SessionError {
	/// An integer code that is sent over the wire.
	pub fn code(&self) -> u64 {
		match self {
			Self::RoleIncompatible(..) => 406,
			Self::RoleViolation => 405,
			Self::WebTransport(_) => 503,
			Self::Version(..) => 406,
			Self::Decode(_) => 400,
			Self::Encode(_) => 500,
			Self::Io(_) => 500,
			Self::BoundsExceeded(_) => 500,
			Self::Duplicate => 409,
			Self::Internal => 500,
			Self::WrongSize => 400,

			Self::Cache(err) => err.code(),
		}
	}
}
