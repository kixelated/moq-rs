use super::SubscribeOrder;
use crate::coding::*;

#[derive(Clone, Debug, Default)]
pub struct Info {
	pub latest: Option<u64>,
	pub default_order: Option<SubscribeOrder>,
	pub default_priority: Option<u64>,
}

impl Encode for Info {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.latest.encode(w)?;
		self.default_order.encode(w)?;
		self.default_priority.encode(w)?;

		Ok(())
	}
}

impl Decode for Info {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let latest = Option::<u64>::decode(r)?;
		let default_order = Option::<SubscribeOrder>::decode(r)?;
		let default_priority = Option::<u64>::decode(r)?;

		Ok(Self {
			latest,
			default_order,
			default_priority,
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
