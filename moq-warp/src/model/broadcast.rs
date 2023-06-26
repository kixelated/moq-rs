use std::{error, fmt};

use moq_transport::VarInt;

// TODO generialize broker::Broadcasts and source::Source into this module.

/*
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
*/

#[derive(Clone)]
pub struct Error {
	pub code: VarInt,
	pub reason: String,
}

impl error::Error for Error {}

impl fmt::Debug for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if !self.reason.is_empty() {
			write!(f, "broadcast error ({}): {}", self.code, self.reason)
		} else {
			write!(f, "broadcast error ({})", self.code)
		}
	}
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if !self.reason.is_empty() {
			write!(f, "broadcast error ({}): {}", self.code, self.reason)
		} else {
			write!(f, "broadcast error ({})", self.code)
		}
	}
}
