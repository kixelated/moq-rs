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
	pub path: Path,
	pub priority: i8,

	pub group_order: group::GroupOrder,
	pub group_expires: time::Duration,
	pub group_min: Option<u64>,
	pub group_max: Option<u64>,
}

impl Decode for Subscribe {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let id = u64::decode(r)?;
		let path = Path::decode(r)?;
		let priority = i8::decode(r)?;

		let group_order = group::GroupOrder::decode(r)?;
		let group_expires = time::Duration::decode(r)?;
		let group_min = match u64::decode(r)? {
			0 => None,
			n => Some(n - 1),
		};
		let group_max = match u64::decode(r)? {
			0 => None,
			n => Some(n - 1),
		};

		Ok(Self {
			id,
			path,
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
		self.path.encode(w);
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
		let priority = u64::decode(r)?;
		let group_order = group::GroupOrder::decode(r)?;
		let group_expires = time::Duration::decode(r)?;
		let group_min = match u64::decode(r)? {
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
