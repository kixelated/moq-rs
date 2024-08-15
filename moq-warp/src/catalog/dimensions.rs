use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Dimensions {
	pub width: u16,
	pub height: u16,
}
