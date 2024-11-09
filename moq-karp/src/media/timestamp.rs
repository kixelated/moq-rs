use std::fmt;

use derive_more::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Rem, RemAssign, Sub, SubAssign, Sum};

#[derive(
	Clone,
	Copy,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	Default,
	Add,
	AddAssign,
	Sub,
	SubAssign,
	Mul,
	MulAssign,
	Div,
	DivAssign,
	Rem,
	RemAssign,
	Sum,
)]
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

	pub fn from_minutes(minutes: u64) -> Self {
		Self {
			micros: minutes * 60_000_000,
		}
	}

	pub fn from_hours(hours: u64) -> Self {
		Self {
			micros: hours * 3_600_000_000,
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

	pub fn as_minutes(&self) -> u64 {
		self.micros / 60_000_000
	}

	pub fn as_hours(&self) -> u64 {
		self.micros / 3_600_000_000
	}

	pub fn as_units(&self, base: u64) -> u64 {
		(self.micros * base) / 1_000_000
	}
}

impl fmt::Debug for Timestamp {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		if self.micros == 0 {
			return write!(f, "0");
		}

		let hours = self.micros / 3_600_000_000;
		let minutes = (self.micros % 3_600_000_000) / 60_000_000;
		let seconds = (self.micros % 60_000_000) / 1_000_000;
		let millis = (self.micros % 1_000_000) / 1_000;
		let micros = self.micros % 1_000;

		let mut parts = Vec::new();
		if hours > 0 {
			parts.push(format!("{}h", hours));
		}
		if minutes > 0 {
			parts.push(format!("{:02}m", minutes));
		}
		if seconds > 0 {
			parts.push(format!("{:02}s", seconds));
		}
		if millis > 0 {
			parts.push(format!("{:03}ms", millis));
		}
		if micros > 0 {
			parts.push(format!("{:03}us", micros));
		}

		write!(f, "{}", parts.join(" "))
	}
}
