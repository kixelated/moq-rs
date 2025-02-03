use derive_more::{From, Into};
use std::{fmt, ops, time::Duration};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, From, Into)]
pub struct Timestamp(Duration);

impl Timestamp {
	pub fn from_micros(micros: u64) -> Self {
		Self(Duration::from_micros(micros))
	}

	pub fn from_millis(millis: u64) -> Self {
		Self(Duration::from_millis(millis))
	}

	pub fn from_secs(seconds: u64) -> Self {
		Self(Duration::from_secs(seconds))
	}

	pub fn from_minutes(minutes: u64) -> Self {
		Self::from_secs(minutes * 60)
	}

	pub fn from_hours(hours: u64) -> Self {
		Self::from_minutes(hours * 60)
	}

	pub fn from_units(value: u64, base: u64) -> Self {
		Self::from_micros((value * 1_000_000) / base)
	}

	pub fn as_micros(self) -> u64 {
		self.0.as_micros() as u64
	}

	pub fn as_millis(self) -> u64 {
		self.0.as_millis() as u64
	}

	pub fn as_secs(self) -> u64 {
		self.0.as_secs()
	}

	pub fn as_minutes(self) -> u64 {
		self.as_secs() / 60
	}

	pub fn as_hours(self) -> u64 {
		self.as_minutes() / 60
	}

	pub fn as_units(self, base: u64) -> u64 {
		(self.as_micros() * base) / 1_000_000
	}
}

impl ops::Deref for Timestamp {
	type Target = Duration;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl ops::Add<Duration> for Timestamp {
	type Output = Timestamp;

	fn add(self, rhs: Duration) -> Self::Output {
		Timestamp(self.0 + rhs)
	}
}

impl ops::Sub<Duration> for Timestamp {
	type Output = Timestamp;

	fn sub(self, rhs: Duration) -> Self::Output {
		Timestamp(self.0 - rhs)
	}
}

impl ops::Sub<Timestamp> for Timestamp {
	type Output = Duration;

	fn sub(self, rhs: Timestamp) -> Self::Output {
		self.0 - rhs.0
	}
}

impl fmt::Debug for Timestamp {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}
