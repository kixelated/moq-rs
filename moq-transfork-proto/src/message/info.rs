use super::GroupOrder;
use crate::coding::*;

#[derive(Clone, Debug)]
pub struct Info {
	pub track_priority: i8,
	pub group_order: GroupOrder,
	pub group_latest: u64,
}

impl Encode for Info {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.track_priority.encode(w);
		self.group_order.encode(w);
		self.group_latest.encode(w);
	}
}

impl Decode for Info {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let track_priority = i8::decode(r)?;
		let group_order = GroupOrder::decode(r)?;
		let group_latest = u64::decode(r)?;

		Ok(Self {
			track_priority,
			group_order,
			group_latest,
		})
	}
}

#[derive(Clone, Debug)]
pub struct InfoRequest {
	pub path: String,
}

impl Encode for InfoRequest {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.path.encode(w);
	}
}

impl Decode for InfoRequest {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let path = String::decode(r)?;
		Ok(Self { path })
	}
}
