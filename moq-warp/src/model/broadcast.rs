use super::{track, watch};

use std::error;

use moq_transport::VarInt;

pub struct Publisher {
	pub namespace: String,

	pub tracks: watch::Publisher<track::Subscriber>,
}

impl Publisher {
	pub fn new(namespace: &str) -> Self {
		Self {
			namespace: namespace.to_string(),
			tracks: watch::Publisher::new(),
		}
	}

	pub fn subscribe(&self) -> Subscriber {
		Subscriber {
			namespace: self.namespace.clone(),
			tracks: self.tracks.subscribe(),
		}
	}
}

#[derive(Clone)]
pub struct Subscriber {
	pub namespace: String,

	pub tracks: watch::Subscriber<track::Subscriber>,
}

pub trait Error: error::Error {
	// Default to error code 1 for unknown errors.
	fn code(&self) -> VarInt {
		VarInt::from_u32(1)
	}
}
