use super::CodecError;
use serde::{Deserialize, Serialize};

#[serde_with::serde_as]
#[serde_with::skip_serializing_none]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Audio {
	pub track: moq_transfork::Track,
	pub codec: AudioCodec,

	// The number of units in a second
	pub timescale: u32,

	pub sample_rate: u16,
	pub channel_count: u16,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub bitrate: Option<u32>,
}

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AudioCodec {
	#[serde(rename = "opus")]
	Opus,
	#[serde(rename = "aac")]
	AAC(AAC),
	#[serde(untagged)]
	Unknown(String),
}

impl std::fmt::Display for AudioCodec {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Opus => write!(f, "opus"),
			Self::AAC(codec) => write!(f, "{}", codec),
			Self::Unknown(codec) => write!(f, "{}", codec),
		}
	}
}

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
	type Err = CodecError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let remain = s.strip_prefix("mp4a.40.").ok_or(CodecError::Invalid)?;
		Ok(Self {
			profile: u8::from_str(remain)?,
		})
	}
}

impl From<AAC> for AudioCodec {
	fn from(codec: AAC) -> Self {
		Self::AAC(codec)
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
