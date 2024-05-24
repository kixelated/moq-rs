use crate::{coding, serve, setup};

#[derive(thiserror::Error, Debug, Clone)]
pub enum SessionError {
	#[error("webtransport session: {0}")]
	Session(#[from] web_transport::SessionError),

	#[error("webtransport write: {0}")]
	Write(#[from] web_transport::WriteError),

	#[error("webtransport read: {0}")]
	Read(#[from] web_transport::ReadError),

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

	#[error("serve error: {0}")]
	Serve(#[from] serve::ServeError),

	#[error("wrong size")]
	WrongSize,
}

impl SessionError {
	/// An integer code that is sent over the wire.
	pub fn code(&self) -> u64 {
		match self {
			Self::RoleIncompatible(..) => 406,
			Self::RoleViolation => 405,
			Self::Session(_) => 503,
			Self::Read(_) => 500,
			Self::Write(_) => 500,
			Self::Version(..) => 406,
			Self::Decode(_) => 400,
			Self::Encode(_) => 500,
			Self::BoundsExceeded(_) => 500,
			Self::Duplicate => 409,
			Self::Internal => 500,
			Self::WrongSize => 400,
			Self::Serve(err) => err.code(),
		}
	}
}

impl From<SessionError> for serve::ServeError {
	fn from(err: SessionError) -> Self {
		match err {
			SessionError::Serve(err) => err,
			_ => serve::ServeError::Internal(err.to_string()),
		}
	}
}
