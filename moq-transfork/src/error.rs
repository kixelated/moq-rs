use crate::{coding, message, setup};

#[derive(thiserror::Error, Debug, Clone)]
pub enum MoqError {
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
	// The broadcast/track is a duplicate
	#[error("duplicate")]
	Duplicate,

	// TODO remove
	//#[error("internal error")]
	//Internal,

	// Cancel is returned when there are no more readers.
	#[error("cancelled")]
	Cancel,

	// The application closes the stream with a code.
	#[error("app code={0}")]
	App(u32),

	#[error("not found")]
	NotFound,

	#[error("wrong frame size")]
	WrongSize,
}

impl MoqError {
	/// An integer code that is sent over the wire.
	pub fn to_code(&self) -> u32 {
		match self {
			Self::Cancel => 0,
			Self::RequiredExtension(_) => 1,
			Self::RoleIncompatible(..) => 2,
			Self::RoleViolation => 3,
			Self::Network(_) => 4,
			Self::Read(_) => 5,
			Self::Decode(_) => 6,
			Self::Write(_) => 7,
			Self::Encode(_) => 8,
			Self::Version(..) => 9,
			Self::UnexpectedStream(_) => 10,
			Self::BoundsExceeded(_) => 11,
			Self::Duplicate => 12,
			Self::NotFound => 13,
			Self::WrongSize => 14,
			Self::App(app) => *app + 64,
		}
	}
}
