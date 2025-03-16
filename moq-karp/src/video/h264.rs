use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::Error;

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct H264 {
	pub profile: u8,
	pub constraints: u8,
	pub level: u8,
}

impl fmt::Display for H264 {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "avc1.{:02x}{:02x}{:02x}", self.profile, self.constraints, self.level)
	}
}

impl FromStr for H264 {
	type Err = Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut parts = s.split('.');
		if parts.next() != Some("avc1") {
			return Err(Error::InvalidCodec);
		}

		let part = parts.next().ok_or(Error::InvalidCodec)?;
		if part.len() != 6 {
			return Err(Error::InvalidCodec);
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
	use std::str::FromStr;

	use crate::VideoCodec;

	use super::*;

	#[test]
	fn test_h264() {
		let encoded = "avc1.42c01e";
		let decoded = H264 {
			profile: 0x42,
			constraints: 0xc0,
			level: 0x1e,
		}
		.into();

		let output = VideoCodec::from_str(encoded).expect("failed to parse");
		assert_eq!(output, decoded);

		let output = decoded.to_string();
		assert_eq!(output, encoded);
	}
}
