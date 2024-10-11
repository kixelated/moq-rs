#[derive(thiserror::Error, Debug)]
pub enum CodecError {
	#[error("invalid codec")]
	Invalid,

	#[error("unsupported codec")]
	Unsupported,

	#[error("expected int")]
	ExpectedInt(#[from] std::num::ParseIntError),
}
