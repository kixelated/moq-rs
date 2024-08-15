use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Eq, Hash)]
pub enum Container {
	Fmp4,
	Loc,
	#[serde(untagged)]
	Unknown(String),
}
