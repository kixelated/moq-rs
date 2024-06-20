use std::time;

use crate::coding::{Decode, DecodeError, Encode, EncodeError};

/// Sent by the subscriber to request all future objects for the given track.
///
/// Objects will use the provided ID instead of the full track name, to save bytes.
#[derive(Clone, Debug)]
pub struct Subscribe {
	pub id: u64,
	pub broadcast: String,

	pub track: String,
	pub priority: u64,

	pub group_order: GroupOrder,
	pub group_expires: Option<time::Duration>,
	pub group_min: Option<u64>,
	pub group_max: Option<u64>,
}

impl Decode for Subscribe {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let id = u64::decode(r)?;
		let broadcast = String::decode(r)?;
		let track = String::decode(r)?;
		let priority = u64::decode(r)?;

		let group_order = GroupOrder::decode(r)?;
		let group_expires = Option::<time::Duration>::decode(r)?;
		let group_min = Option::<u64>::decode(r)?;
		let group_max = Option::<u64>::decode(r)?;

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
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w)?;
		self.broadcast.encode(w)?;
		self.track.encode(w)?;
		self.priority.encode(w)?;

		self.group_order.encode(w)?;
		self.group_expires.encode(w)?;
		self.group_min.encode(w)?;
		self.group_max.encode(w)?;

		Ok(())
	}
}

#[derive(Clone, Debug)]
pub struct SubscribeUpdate {
	pub priority: u64,

	pub group_order: GroupOrder,
	pub group_expires: Option<time::Duration>,
	pub group_min: Option<u64>,
	pub group_max: Option<u64>,
}

impl Decode for SubscribeUpdate {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let priority = u64::decode(r)?;
		let group_order = GroupOrder::decode(r)?;
		let group_expires = Option::<time::Duration>::decode(r)?;
		let group_min = Option::<u64>::decode(r)?;
		let group_max = Option::<u64>::decode(r)?;

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
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.priority.encode(w)?;
		self.group_order.encode(w)?;
		self.group_min.encode(w)?;
		self.group_max.encode(w)?;

		Ok(())
	}
}

#[derive(Clone, Debug, Copy)]
pub enum GroupOrder {
	Ascending,
	Descending,
}

impl Decode for GroupOrder {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		match u64::decode(r)? {
			0 => Ok(GroupOrder::Ascending),
			1 => Ok(GroupOrder::Descending),
			_ => Err(DecodeError::InvalidValue),
		}
	}
}

impl Encode for GroupOrder {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		let v: u64 = match self {
			GroupOrder::Ascending => 0,
			GroupOrder::Descending => 1,
		};
		v.encode(w)
	}
}
