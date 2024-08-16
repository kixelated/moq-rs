use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Eq, Hash)]
pub enum Container {
	#[serde(rename = "fmp4")]
	Fmp4,

	#[serde(rename = "loc")]
	Loc,

	#[serde(untagged)]
	Unknown(String),
}
