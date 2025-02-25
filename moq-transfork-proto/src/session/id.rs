use derive_more::{From, Into};

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone, PartialOrd, Ord)]
pub struct AnnounceId(u64);

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone, PartialOrd, Ord)]
pub struct SubscribeId(u64);

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone, PartialOrd, Ord)]
pub struct GroupId(u64);

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone, PartialOrd, Ord)]
pub struct FrameId(u64);

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone, From, Into, PartialOrd, Ord)]
pub struct StreamId(u64);

impl StreamId {
	pub fn is_bi(&self) -> bool {
		self.0 & 0b10 == 0
	}

	pub fn is_uni(&self) -> bool {
		!self.is_bi()
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

impl Increment for FrameId {
	fn increment(&mut self) {
		self.0 += 1;
	}
}

impl Increment for StreamId {
	fn increment(&mut self) {
		self.0 += 4;
	}
}
