use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::catalog::CodecError;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct H265 {
	pub profile: u8,
	pub constraints: u8,
	pub level: u8,
}

impl H265 {
	pub const PREFIX: &'static str = "hev1";
}

impl fmt::Display for H265 {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "hev1.{:02x}{:02x}{:02x}", self.profile, self.constraints, self.level)
	}
}

impl FromStr for H265 {
	type Err = CodecError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut parts = s.split('.');
		if parts.next() != Some("hev1") {
			return Err(CodecError::Invalid);
		}

		let part = parts.next().ok_or(CodecError::Invalid)?;
		if part.len() != 6 {
			return Err(CodecError::Invalid);
		}

		Ok(Self {
			profile: u8::from_str_radix(&part[0..2], 16)?,
			constraints: u8::from_str_radix(&part[2..4], 16)?,
			level: u8::from_str_radix(&part[4..6], 16)?,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_h265() {
		let encoded = "hev1.42c01e";
		let decoded = H265 {
			profile: 0x42,
			constraints: 0xc0,
			level: 0x1e,
		};

		let output = H265::from_str(encoded).expect("failed to parse");
		assert_eq!(output, decoded);

		let output = decoded.to_string();
		assert_eq!(output, encoded);
	}
}
