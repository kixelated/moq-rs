use serde::{Deserialize, Serialize};

use super::CodecError;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct VP8;

impl VP8 {
	pub const PREFIX: &'static str = "vp8";
}

impl std::fmt::Display for VP8 {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "vp8")
	}
}

impl std::str::FromStr for VP8 {
	type Err = CodecError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		if s != "vp8" {
			return Err(CodecError::Invalid);
		}

		Ok(Self)
	}
}
