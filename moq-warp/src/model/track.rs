use super::{segment, watch};
use std::{error, fmt};

use moq_transport::VarInt;

pub struct Publisher {
	pub name: String,

	pub segments: watch::Publisher<segment::Subscriber>,
}

impl Publisher {
	pub fn new(name: &str) -> Publisher {
		Self {
			name: name.to_string(),
			segments: watch::Publisher::new(),
		}
	}

	pub fn subscribe(&self) -> Subscriber {
		Subscriber {
			name: self.name.clone(),
			segments: self.segments.subscribe(),
		}
	}
}

impl fmt::Debug for Publisher {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "track publisher: {:?}", self.name)
	}
}

#[derive(Clone)]
pub struct Subscriber {
	pub name: String,

	// A list of segments, which are independently decodable.
	pub segments: watch::Subscriber<segment::Subscriber>,
}

impl fmt::Debug for Subscriber {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "track subscriber: {:?}", self.name)
	}
}

pub trait Error: error::Error {
	// Default to error code 1 for unknown errors.
	fn code(&self) -> VarInt {
		VarInt::from_u32(1)
	}
}
