#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("json error: {0}")]
	Json(#[from] serde_json::Error),

	#[error("moq error: {0}")]
	Moq(#[from] moq_transfork::Error),

	#[error("empty catalog")]
	Empty,

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
