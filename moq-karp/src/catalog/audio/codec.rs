use super::*;

use derive_more::{Display, From};
use std::str::FromStr;

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Display, From)]
pub enum AudioCodec {
	Opus,
	AAC(AAC),

	#[serde(untagged)]
	Unknown(String),
}

impl FromStr for AudioCodec {
	type Err = Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		if s.starts_with("mp4a.40.") {
			return AAC::from_str(s).map(Into::into);
		} else if s == "opus" {
			return Ok(Self::Opus);
		}

		Ok(Self::Unknown(s.to_string()))
	}
}
