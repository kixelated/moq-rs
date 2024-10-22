use std::time;

use super::GroupOrder;
use crate::coding::*;
use crate::Path;

#[derive(Clone, Debug)]
pub struct Info {
	pub track_priority: i8,
	pub group_order: GroupOrder,
	pub group_expires: time::Duration,
	pub group_latest: u64,
}

impl Encode for Info {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.track_priority.encode(w);
		self.group_order.encode(w);
		self.group_expires.encode(w);
		self.group_latest.encode(w);
	}
}

impl Decode for Info {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let track_priority = i8::decode_more(r, 3)?;
		let group_order = GroupOrder::decode_more(r, 2)?;
		let group_expires = time::Duration::decode_more(r, 1)?;
		let group_latest = u64::decode(r)?;

		Ok(Self {
			track_priority,
			group_order,
			group_expires,
			group_latest,
		})
	}
}

#[derive(Clone, Debug)]
pub struct InfoRequest {
	pub broadcast: Path,
	pub track: String,
}

impl Encode for InfoRequest {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.broadcast.encode(w);
		self.track.encode(w);
	}
}

impl Decode for InfoRequest {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let broadcast = Path::decode_more(r, 1)?;
		let track = String::decode(r)?;

		Ok(Self { broadcast, track })
	}
}
