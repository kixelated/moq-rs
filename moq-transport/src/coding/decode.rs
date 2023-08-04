use super::VarInt;
use std::str;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DecodeError {
	#[error("unexpected end of buffer")]
	UnexpectedEnd,

	#[error("invalid string")]
	InvalidString(#[from] str::Utf8Error),

	#[error("invalid type: {0:?}")]
	InvalidType(VarInt),

	#[error("io error: {0}")]
	IoError(#[from] std::io::Error),
}
