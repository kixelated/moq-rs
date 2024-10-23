use std::time;

use crate::{
	coding::{Decode, DecodeError, Encode},
	message::group,
	Path,
};

/// Sent by the subscriber to request all future objects for the given track.
///
/// Objects will use the provided ID instead of the full track name, to save bytes.
#[derive(Clone, Debug)]
pub struct Subscribe {
	pub id: u64,
	pub broadcast: Path,

	pub track: String,
	pub priority: i8,

	pub group_order: group::GroupOrder,
	pub group_expires: time::Duration,
	pub group_min: Option<u64>,
	pub group_max: Option<u64>,
}

impl Decode for Subscribe {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let id = u64::decode_more(r, 6)?;
		let broadcast = Path::decode_more(r, 5)?;
		let track = String::decode_more(r, 4)?;
		let priority = i8::decode_more(r, 4)?;

		let group_order = group::GroupOrder::decode_more(r, 3)?;
		let group_expires = time::Duration::decode_more(r, 2)?;
		let group_min = match u64::decode_more(r, 1)? {
			0 => None,
			n => Some(n - 1),
		};
		let group_max = match u64::decode(r)? {
			0 => None,
			n => Some(n - 1),
		};

		Ok(Self {
			id,
			broadcast,
			track,
			priority,

			group_order,
			group_expires,
			group_min,
			group_max,
		})
	}
}

impl Encode for Subscribe {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.id.encode(w);
		self.broadcast.encode(w);
		self.track.encode(w);
		self.priority.encode(w);

		self.group_order.encode(w);
		self.group_expires.encode(w);
		self.group_min.map(|v| v + 1).unwrap_or(0).encode(w);
		self.group_max.map(|v| v + 1).unwrap_or(0).encode(w);
	}
}

#[derive(Clone, Debug)]
pub struct SubscribeUpdate {
	pub priority: u64,

	pub group_order: group::GroupOrder,
	pub group_expires: time::Duration,
	pub group_min: Option<u64>,
	pub group_max: Option<u64>,
}

impl Decode for SubscribeUpdate {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let priority = u64::decode_more(r, 4)?;
		let group_order = group::GroupOrder::decode_more(r, 3)?;
		let group_expires = time::Duration::decode_more(r, 2)?;
		let group_min = match u64::decode_more(r, 1)? {
			0 => None,
			n => Some(n - 1),
		};
		let group_max = match u64::decode(r)? {
			0 => None,
			n => Some(n - 1),
		};

		Ok(Self {
			priority,
			group_order,
			group_expires,
			group_min,
			group_max,
		})
	}
}

impl Encode for SubscribeUpdate {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.priority.encode(w);
		self.group_order.encode(w);
		self.group_min.map(|v| v + 1).unwrap_or(0).encode(w);
		self.group_max.map(|v| v + 1).unwrap_or(0).encode(w);
	}
}
