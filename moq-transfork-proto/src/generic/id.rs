use derive_more::{From, Into};

use super::StreamDirection;

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone, PartialOrd, Ord)]
pub struct AnnounceId(pub u64);

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone, PartialOrd, Ord)]
pub struct SubscribeId(pub u64);

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone, PartialOrd, Ord)]
pub struct GroupId(pub u64);

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone, From, Into, PartialOrd, Ord)]
pub struct StreamId(pub u64);

impl StreamId {
	pub fn is_bi(&self) -> bool {
		self.0 & 0b10 == 0
	}

	pub fn is_uni(&self) -> bool {
		!self.is_bi()
	}

	pub fn direction(&self) -> StreamDirection {
		if self.is_bi() {
			StreamDirection::Bi
		} else {
			StreamDirection::Uni
		}
	}
}

pub(super) trait Increment {
	fn increment(&mut self);
}

impl Increment for AnnounceId {
	fn increment(&mut self) {
		self.0 += 1;
	}
}

impl Increment for SubscribeId {
	fn increment(&mut self) {
		self.0 += 1;
	}
}

impl Increment for GroupId {
	fn increment(&mut self) {
		self.0 += 1;
	}
}

impl Increment for StreamId {
	fn increment(&mut self) {
		self.0 += 4;
	}
}
