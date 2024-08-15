#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("json error: {0}")]
	Json(#[from] serde_json::Error),

	#[error("moq error: {0}")]
	Moq(#[from] moq_transfork::Error),

	#[error("empty catalog")]
	Empty,

	#[error("codec error: {0}")]
	Codec(#[from] CodecError),

	#[error("hex error: {0}")]
	Hex(#[from] hex::FromHexError),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum CodecError {
	#[error("invalid codec")]
	Invalid,

	#[error("unsupported codec")]
	Unsupported,

	#[error("expected int")]
	ExpectedInt(#[from] std::num::ParseIntError),
}
