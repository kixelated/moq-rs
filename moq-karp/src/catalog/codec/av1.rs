use serde::{Deserialize, Serialize};

use super::CodecError;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AV1 {
	pub profile: u8,
	pub level: u8,
	pub tier: char,
	pub bitdepth: u8,
	pub mono_chrome: bool,
	pub chroma_subsampling: u8, // TODO this might need to be a string or enum?
	pub color_primaries: u8,
	pub transfer_characteristics: u8,
	pub matrix_coefficients: u8,
	pub full_range: bool,
}

impl AV1 {
	pub const PREFIX: &'static str = "av01";
}

// av01.<profile>.<level><tier>.<bitDepth>.<monochrome>.<chromaSubsampling>.
// <colorPrimaries>.<transferCharacteristics>.<matrixCoefficients>.<videoFullRangeFlag>
impl std::fmt::Display for AV1 {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"av01.{:01}.{:02}{}.{:02}",
			self.profile, self.level, self.tier, self.bitdepth
		)?;

		let short = AV1 {
			profile: self.profile,
			level: self.level,
			tier: self.tier,
			bitdepth: self.bitdepth,
			..Default::default()
		};

		if self == &short {
			return Ok(());
		}

		write!(
			f,
			".{:01}.{:03}.{:02}.{:02}.{:02}.{:01}",
			self.mono_chrome as u8,
			self.chroma_subsampling,
			self.color_primaries,
			self.transfer_characteristics,
			self.matrix_coefficients,
			self.full_range as u8,
		)
	}
}

lazy_static::lazy_static! {
	static ref AV1_REGEX: regex::Regex = regex::Regex::new(&r#"
		^av01
		\.(?<profile>[01])
		\.(?<level>\d{2})(?<tier>\w)
		\.(?<bitdepth>\d{2})
		(?<extra>
			\.(?<mono>\d)
			\.(?<chroma>\d{3})
			\.(?<color>\d{2})
			\.(?<transfer>\d{2})
			\.(?<matrix>\d{2})
			\.(?<full>[01])
		)?
		$"#.replace(['\n', ' ', '\t'], "")
	).unwrap();
}

impl std::str::FromStr for AV1 {
	type Err = CodecError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let captures = AV1_REGEX.captures(s).ok_or(CodecError::Invalid)?;

		let mut av1 = AV1 {
			profile: u8::from_str(&captures["profile"])?,
			level: u8::from_str(&captures["level"])?,
			tier: captures["tier"].chars().next().unwrap(),
			bitdepth: u8::from_str(&captures["bitdepth"])?,
			..Default::default()
		};

		if captures.name("extra").is_none() {
			return Ok(av1);
		}

		av1.mono_chrome = &captures["mono"] == "1";
		av1.chroma_subsampling = u8::from_str(&captures["chroma"])?;
		av1.color_primaries = u8::from_str(&captures["color"])?;
		av1.transfer_characteristics = u8::from_str(&captures["transfer"])?;
		av1.matrix_coefficients = u8::from_str(&captures["matrix"])?;
		av1.full_range = &captures["full"] == "1";

		Ok(av1)
	}
}

impl Default for AV1 {
	fn default() -> Self {
		// .0.110.01.01.01.0
		Self {
			profile: 0,
			level: 0,
			tier: 'M',
			bitdepth: 8,
			mono_chrome: false,
			chroma_subsampling: 110,
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
	fn test_av1() {
		let encoded = "av01.0.04M.10.0.112.09.16.09.0";
		let decoded = AV1 {
			profile: 0,
			level: 4,
			tier: 'M',
			bitdepth: 10,
			mono_chrome: false,
			chroma_subsampling: 112,
			color_primaries: 9,
			transfer_characteristics: 16,
			matrix_coefficients: 9,
			full_range: false,
		};

		let output = AV1::from_str(encoded).expect("failed to parse");
		assert_eq!(output, decoded);

		let output = decoded.to_string();
		assert_eq!(output, encoded);
	}

	#[test]
	fn test_av1_short() {
		let encoded = "av01.0.01M.08";
		let decoded = AV1 {
			profile: 0,
			level: 1,
			tier: 'M',
			bitdepth: 8,
			..Default::default()
		};

		let output = AV1::from_str(encoded).expect("failed to parse");
		assert_eq!(output, decoded);

		let output = decoded.to_string();
		assert_eq!(output, encoded);
	}
}
