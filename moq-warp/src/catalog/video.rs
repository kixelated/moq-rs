use serde::{Deserialize, Serialize};
use serde_with::DisplayFromStr;

use super::{CodecError, Container, Dimensions};

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Video {
	pub track: moq_transfork::Track,
	pub container: Container,

	#[serde_as(as = "DisplayFromStr")]
	pub codec: VideoCodec,

	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub bitrate: Option<u32>,

	pub dimensions: Dimensions,

	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub display: Option<Dimensions>,

	// Additional enhancement layers
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub layers: Vec<VideoLayer>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VideoLayer {
	pub track: moq_transfork::Track,
	pub dimensions: Dimensions,
}

macro_rules! video_codec {
	{$($name:ident = $prefix:expr,)*} => {
		#[serde_with::serde_as]
		#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
		pub enum VideoCodec {
			$($name($name),)*

			#[serde(untagged)]
			Unknown(String),
		}

		$(
			impl From<$name> for VideoCodec {
				fn from(codec: $name) -> Self {
					Self::$name(codec)
				}
			}
		)*

		impl std::fmt::Display for VideoCodec {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				match self {
					$(
						Self::$name(codec) => write!(f, "{}", codec),
					)*
					Self::Unknown(codec) => write!(f, "{}", codec),
				}
			}
		}

		impl std::str::FromStr for VideoCodec {
			type Err = CodecError;

			fn from_str(s: &str) -> Result<Self, Self::Err> {
				$(
					if s.starts_with($prefix) {
						return Ok(Self::$name($name::from_str(s)?));
					}
				)*
				Ok(Self::Unknown(s.into()))
			}
		}
	};
}

video_codec! {
	H264 = "avc1",
	H265 = "hev1",
	VP8 = "vp8",
	VP9 = "vp09",
	AV1 = "av01",
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct H264 {
	pub profile: u8,
	pub constraints: u8,
	pub level: u8,
}

impl std::fmt::Display for H264 {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "avc1.{:02x}{:02x}{:02x}", self.profile, self.constraints, self.level)
	}
}

impl std::str::FromStr for H264 {
	type Err = CodecError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut parts = s.split('.');
		if parts.next() != Some("avc1") {
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct H265 {
	pub profile: u8,
	pub constraints: u8,
	pub level: u8,
}

impl std::fmt::Display for H265 {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "hev1.{:02x}{:02x}{:02x}", self.profile, self.constraints, self.level)
	}
}

impl std::str::FromStr for H265 {
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct VP8;

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

// vp09.<profile>.<level>.<bitDepth>.<chromaSubsampling>.
// <colourPrimaries>.<transferCharacteristics>.<matrixCoefficients>.<videoFullRangeFlag>
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
			.map(|part| u8::from_str(part))
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

// av01.<profile>.<level><tier>.<bitDepth>.<monochrome>.<chromaSubsampling>.
// <colorPrimaries>.<transferCharacteristics>.<matrixCoefficients>.<videoFullRangeFlag>
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
		$"#.replace("\n", "").replace(" ", "").replace("\t", "")
	).unwrap();
}

impl std::str::FromStr for AV1 {
	type Err = CodecError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		println!("{}", AV1_REGEX.as_str());
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
mod tests {
	use super::*;

	use std::str::FromStr;

	#[test]
	fn test_h264() {
		let encoded = "avc1.42c01e";
		let decoded = H264 {
			profile: 0x42,
			constraints: 0xc0,
			level: 0x1e,
		};

		let output = H264::from_str(encoded).expect("failed to parse");
		assert_eq!(output, decoded);

		let output = decoded.to_string();
		assert_eq!(output, encoded);
	}

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
