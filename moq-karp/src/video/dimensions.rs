use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Dimensions {
	pub width: u32,
	pub height: u32,
}
