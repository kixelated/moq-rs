#[derive(Debug, Clone, Copy)]
pub struct Timestamp {
	pub micros: u64,
}

impl Timestamp {
	pub fn from_micros(micros: u64) -> Self {
		Self { micros }
	}

	pub fn from_millis(millis: u64) -> Self {
		Self { micros: millis * 1_000 }
	}

	pub fn from_seconds(seconds: u64) -> Self {
		Self {
			micros: seconds * 1_000_000,
		}
	}

	pub fn from_scale(base: u64, scale: u64) -> Self {
		Self {
			micros: base * 1_000_000 / scale,
		}
	}

	pub fn as_micros(&self) -> u64 {
		self.micros
	}

	pub fn as_millis(&self) -> u64 {
		self.micros / 1_000
	}

	pub fn as_seconds(&self) -> u64 {
		self.micros / 1_000_000
	}

	pub fn to_scale(&self, scale: u64) -> u64 {
		self.micros * scale / 1_000_000
	}
}
