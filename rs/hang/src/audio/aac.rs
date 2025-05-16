use super::Error;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AAC {
	pub profile: u8,
	// freq_index
	// chan_conf
}

impl std::fmt::Display for AAC {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "mp4a.40.{}", self.profile)
	}
}

impl std::str::FromStr for AAC {
	type Err = Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let remain = s.strip_prefix("mp4a.40.").ok_or(Error::InvalidCodec)?;
		Ok(Self {
			profile: u8::from_str(remain)?,
		})
	}
}

#[cfg(test)]
mod test {
	use std::str::FromStr;

	use super::*;

	#[test]
	fn test_aac() {
		let encoded = "mp4a.40.2";
		let decoded = AAC { profile: 2 };

		let output = AAC::from_str(encoded).expect("failed to parse AAC string");
		assert_eq!(output, decoded);

		let output = decoded.to_string();
		assert_eq!(output, encoded);
	}
}
