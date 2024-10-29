#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp {
	micros: u64,
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

	pub fn from_units(value: u64, base: u64) -> Self {
		Self {
			micros: (value * 1_000_000) / base,
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

	pub fn as_units(&self, base: u64) -> u64 {
		(self.micros * base) / 1_000_000
	}
}
