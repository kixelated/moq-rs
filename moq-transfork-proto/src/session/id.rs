use derive_more::{From, Into};

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone)]
pub struct AnnounceId(u64);

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone)]
pub struct SubscribeId(u64);

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone)]
pub struct GroupId(u64);

#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone)]
pub struct FrameId(u64);

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct StreamId {
	pub remote: bool,
	pub bidi: bool,
	pub id: u64,
}
