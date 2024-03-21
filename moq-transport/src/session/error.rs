use crate::{coding, serve, setup};

#[derive(thiserror::Error, Debug, Clone)]
pub enum SessionError<S: webtransport_generic::Session> {
	// We can't use #[from] here because it would conflict with
	#[error("webtransport error: {0}")]
	WebTransport(S::Error),

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

impl<S: webtransport_generic::Session> SessionError<S> {
	/// An integer code that is sent over the wire.
	pub fn code(&self) -> u64 {
		match self {
			Self::RoleIncompatible(..) => 406,
			Self::RoleViolation => 405,
			Self::WebTransport(_) => 503,
			Self::Version(..) => 406,
			Self::Decode(_) => 400,
			Self::Encode(_) => 500,
			Self::BoundsExceeded(_) => 500,
			Self::Duplicate => 409,
			Self::Internal => 500,
			Self::WrongSize => 400,

			Self::Cache(err) => err.code(),
		}
	}
}
