use serde::{Deserialize, Serialize};

use super::CodecError;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct VP9 {
	pub profile: u8,
	pub level: u8,
	pub bit_depth: u8,
	pub chroma_subsampling: u8,
	pub color_primaries: u8,
	pub transfer_characteristics: u8,
	pub matrix_coefficients: u8,
	pub full_range: bool,
}

impl VP9 {
	pub const PREFIX: &'static str = "vp09";
}

// vp09.<profile>.<level>.<bitDepth>.<chromaSubsampling>.
// <colourPrimaries>.<transferCharacteristics>.<matrixCoefficients>.<videoFullRangeFlag>
impl std::fmt::Display for VP9 {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "vp09.{:02}.{:02}.{:02}", self.profile, self.level, self.bit_depth)?;

		let short = VP9 {
			profile: self.profile,
			level: self.level,
			bit_depth: self.bit_depth,
			..Default::default()
		};

		if self == &short {
			return Ok(());
		}

		write!(
			f,
			".{:02}.{:02}.{:02}.{:02}.{:02}",
			self.chroma_subsampling,
			self.color_primaries,
			self.transfer_characteristics,
			self.matrix_coefficients,
			self.full_range as u8,
		)
	}
}

impl std::str::FromStr for VP9 {
	type Err = CodecError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let parts = s
			.strip_prefix("vp09.")
			.ok_or(CodecError::Invalid)?
			.split('.')
			.map(u8::from_str)
			.collect::<Result<Vec<_>, _>>()?;

		if parts.len() < 3 {
			return Err(CodecError::Invalid);
		}

		let mut vp9 = VP9 {
			profile: parts[0],
			level: parts[1],
			bit_depth: parts[2],
			..Default::default()
		};

		if parts.len() == 3 {
			return Ok(vp9);
		} else if parts.len() != 8 {
			return Err(CodecError::Invalid);
		}

		vp9.chroma_subsampling = parts[3];
		vp9.color_primaries = parts[4];
		vp9.transfer_characteristics = parts[5];
		vp9.matrix_coefficients = parts[6];
		vp9.full_range = parts[7] == 1;

		Ok(vp9)
	}
}

impl Default for VP9 {
	fn default() -> Self {
		Self {
			profile: 0,
			level: 0,
			bit_depth: 0,
			chroma_subsampling: 1,
			color_primaries: 1,
			transfer_characteristics: 1,
			matrix_coefficients: 1,
			full_range: false,
		}
	}
}

#[cfg(test)]
mod test {
	use std::str::FromStr;

	use super::*;

	#[test]
	fn test_vp9() {
		let encoded = "vp09.02.10.10.01.09.16.09.01";
		let decoded = VP9 {
			profile: 2,
			level: 10,
			bit_depth: 10,
			chroma_subsampling: 1,
			color_primaries: 9,
			transfer_characteristics: 16,
			matrix_coefficients: 9,
			full_range: true,
		};

		let output = VP9::from_str(encoded).expect("failed to parse");
		assert_eq!(output, decoded);

		let output = decoded.to_string();
		assert_eq!(output, encoded);
	}

	#[test]
	fn test_vp9_short() {
		let encoded = "vp09.00.41.08";
		let decoded = VP9 {
			profile: 0,
			level: 41,
			bit_depth: 8,
			..Default::default()
		};

		let output = VP9::from_str(encoded).expect("failed to parse");
		assert_eq!(output, decoded);

		let output = decoded.to_string();
		assert_eq!(output, encoded);
	}
}
