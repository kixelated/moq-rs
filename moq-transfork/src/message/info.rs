use std::time;

use super::SubscribeOrder;
use crate::coding::*;

#[derive(Clone, Debug, Default)]
pub struct Info {
	pub latest: Option<u64>,
	pub priority: Option<u64>,
	pub group_order: Option<SubscribeOrder>,
	pub group_expires: Option<time::Duration>,
}

impl Encode for Info {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.latest.encode(w)?;
		self.priority.encode(w)?;
		self.group_order.encode(w)?;
		self.group_expires.encode(w)?;

		Ok(())
	}
}

impl Decode for Info {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let latest = Option::<u64>::decode(r)?;
		let priority = Option::<u64>::decode(r)?;
		let group_order = Option::<SubscribeOrder>::decode(r)?;
		let group_expires = Option::<time::Duration>::decode(r)?;

		Ok(Self {
			latest,
			priority,
			group_order,
			group_expires,
		})
	}
}

pub struct InfoRequest {
	pub broadcast: String,
	pub track: String,
}

impl Encode for InfoRequest {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.broadcast.encode(w)?;
		self.track.encode(w)?;

		Ok(())
	}
}

impl Decode for InfoRequest {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let broadcast = String::decode(r)?;
		let track = String::decode(r)?;

		Ok(Self { broadcast, track })
	}
}
