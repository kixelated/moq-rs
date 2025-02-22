use derive_more::{From, Into};

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone)]
pub struct AnnounceId(u64);

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone)]
pub struct SubscribeId(u64);

impl SubscribeId {
	pub(crate) fn incr(&mut self) {
		self.0 += 1
	}
}

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone)]
pub struct GroupId(u64);

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone)]
pub struct FrameId(u64);

#[derive(From, Into, Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct StreamId(u64);

impl StreamId {
	pub fn is_bi(&self) -> bool {
		self.0 & 0b10 == 0
	}

	pub fn is_uni(&self) -> bool {
		!self.is_bi()
	}
}
