use moq_transfork_proto::message;

/// A list of possible errors that can occur during the session.
#[derive(thiserror::Error, Debug, Clone)]
pub enum Error {
	#[error("webtransport error: {0}")]
	WebTransport(#[from] web_transport::Error),

	#[error("decode error: {0}")]
	Decode(#[from] moq_transfork_proto::coding::DecodeError),

	/// Some VarInt was too large and we were too lazy to handle it
	#[error("varint bounds exceeded")]
	BoundsExceeded(#[from] moq_transfork_proto::coding::BoundsExceeded),

	#[error("not found")]
	NotFound,

	// Cancel is returned when there are no more readers.
	#[error("cancelled")]
	Cancel,

	/// A duplicate ID was used
	// The broadcast/track is a duplicate
	#[error("duplicate")]
	Duplicate,

	// The application closes the stream with a code.
	#[error("app code={0}")]
	App(u32),

	#[error("wrong frame size")]
	WrongSize,

	#[error("protocol violation")]
	ProtocolViolation,

	/// An unexpected stream was received
	#[error("unexpected stream: {0:?}")]
	UnexpectedStream(message::ControlType),

	// TODO move to a ConnectError
	#[error("unsupported versions: client={0:?} server={1:?}")]
	Version(message::Versions, message::Versions),
}

pub type Result<T> = std::result::Result<T, Error>;
