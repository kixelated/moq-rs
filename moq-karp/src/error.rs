#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("transfork error: {0}")]
	Transfork(#[from] moq_transfork::Error),

	#[error("decode error: {0}")]
	Decode(#[from] moq_transfork::coding::DecodeError),

	#[error("json error: {0}")]
	Json(#[from] serde_json::Error),

	#[error("duplicate track")]
	DuplicateTrack,

	#[error("missing track")]
	MissingTrack,

	#[error("invalid session ID")]
	InvalidSession,

	#[error("empty group")]
	EmptyGroup,

	#[error("invalid codec")]
	InvalidCodec,

	#[error("unsupported codec")]
	UnsupportedCodec,

	#[error("expected int")]
	ExpectedInt(#[from] std::num::ParseIntError),

	#[error("hex error: {0}")]
	Hex(#[from] hex::FromHexError),
}

pub type Result<T> = std::result::Result<T, Error>;
