use crate::{coding, message, model, setup};

#[derive(thiserror::Error, Debug, Clone)]
pub enum SessionError {
	#[error("webtransport session: {0}")]
	Network(#[from] web_transport::SessionError),

	#[error("write error: {0}")]
	Write(#[from] web_transport::WriteError),

	#[error("read error: {0}")]
	Read(#[from] web_transport::ReadError),

	#[error("decode error: {0}")]
	Decode(#[from] coding::DecodeError),

	#[error("encode error: {0}")]
	Encode(#[from] coding::EncodeError),

	// TODO move to a ConnectError
	#[error("unsupported versions: client={0:?} server={1:?}")]
	Version(setup::Versions, setup::Versions),

	// TODO move to a ConnectError
	#[error("incompatible roles: client={0:?} server={1:?}")]
	RoleIncompatible(setup::Role, setup::Role),

	/// The role negiotiated in the handshake was violated. For example, a publisher sent a SUBSCRIBE, or a subscriber sent an OBJECT.
	#[error("role violation")]
	RoleViolation,

	/// A required extension was not present
	#[error("extension required: {0}")]
	RequiredExtension(u64),

	/// An unexpected stream was received
	#[error("unexpected stream: {0:?}")]
	UnexpectedStream(message::Stream),

	/// Some VarInt was too large and we were too lazy to handle it
	#[error("varint bounds exceeded")]
	BoundsExceeded(#[from] coding::BoundsExceeded),

	/// A duplicate ID was used
	#[error("duplicate")]
	Duplicate,

	#[error("internal error")]
	Internal,

	#[error("closed: {0}")]
	Closed(#[from] model::Closed),
}

impl SessionError {
	/// An integer code that is sent over the wire.
	pub fn code(&self) -> u32 {
		match self {
			Self::RequiredExtension(_) => 407,
			Self::RoleIncompatible(..) => 406,
			Self::RoleViolation => 405,
			Self::Network(_) => 503,
			Self::Read(_) => 400,
			Self::Decode(_) => 401,
			Self::Write(_) => 500,
			Self::Encode(_) => 501,
			Self::Version(..) => 406,
			Self::UnexpectedStream(_) => 500,
			Self::BoundsExceeded(_) => 500,
			Self::Duplicate => 409,
			Self::Internal => 500,
			Self::Closed(err) => err.code(),
		}
	}
}

pub(crate) trait Close {
	fn close(&mut self, err: SessionError);
}

pub(crate) trait OrClose<S: Close, V> {
	fn or_close(self, stream: &mut S) -> Result<V, SessionError>;
}

impl<S: Close, V> OrClose<S, V> for Result<V, SessionError> {
	fn or_close(self, stream: &mut S) -> Result<V, SessionError> {
		match self {
			Ok(v) => Ok(v),
			Err(err) => {
				stream.close(err.clone());
				Err(err)
			}
		}
	}
}
