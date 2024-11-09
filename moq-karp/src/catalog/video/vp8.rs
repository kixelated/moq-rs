use serde::{Deserialize, Serialize};

use crate::catalog::Error;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct VP8;

impl std::fmt::Display for VP8 {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "vp8")
	}
}

impl std::str::FromStr for VP8 {
	type Err = Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		if s != "vp8" {
			return Err(Error::InvalidCodec);
		}

		Ok(Self)
	}
}
