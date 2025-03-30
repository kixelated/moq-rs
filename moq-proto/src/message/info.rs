use super::GroupOrder;
use crate::coding::*;

#[derive(Clone, Debug)]
pub struct Info {
	pub priority: i8,
	pub order: GroupOrder,
	pub latest: u64,
}

impl Encode for Info {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.priority.encode(w);
		self.order.encode(w);
		self.latest.encode(w);
	}
}

impl Decode for Info {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let priority = i8::decode(r)?;
		let order = GroupOrder::decode(r)?;
		let latest = u64::decode(r)?;

		Ok(Self {
			priority,
			order,
			latest,
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
