#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamp {
	pub base: u64,
	pub scale: u64,
}

impl From<Timestamp> for std::time::Duration {
	// TOOD untested
	fn from(timestamp: Timestamp) -> Self {
		let seconds = timestamp.base / timestamp.scale;
		let nanos = (timestamp.base % timestamp.scale) * 1_000_000_000 / timestamp.scale;

		Self::new(seconds, nanos as u32)
	}
}

// TODO implement Ord
