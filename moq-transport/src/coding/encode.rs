use super::BoundsExceeded;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum EncodeError {
	#[error("varint too large")]
	BoundsExceeded(#[from] BoundsExceeded),

	#[error("i/o error: {0}")]
	IoError(#[from] std::io::Error),
}
