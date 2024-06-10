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

	pub group_order: Option<SubscribeOrder>,
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
		let order = Option::<SubscribeOrder>::decode(r)?;
		let expires = Option::<time::Duration>::decode(r)?;

		let min = Option::<u64>::decode(r)?;
		let max = Option::<u64>::decode(r)?;

		Ok(Self {
			id,
			broadcast,
			track,
			priority,
			group_order: order,
			group_expires: expires,
			group_min: min,
			group_max: max,
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
	pub order: Option<SubscribeOrder>,
	pub expires: Option<u64>,
	pub min: Option<u64>,
	pub max: Option<u64>,
}

impl Decode for SubscribeUpdate {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let priority = u64::decode(r)?;
		let order = Option::<SubscribeOrder>::decode(r)?;
		let expires = Option::<u64>::decode(r)?;

		let min = Option::<u64>::decode(r)?;
		let max = Option::<u64>::decode(r)?;

		Ok(Self {
			priority,
			order,
			expires,
			min,
			max,
		})
	}
}

impl Encode for SubscribeUpdate {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.priority.encode(w)?;
		self.order.encode(w)?;
		self.expires.encode(w)?;
		self.min.encode(w)?;
		self.max.encode(w)?;

		Ok(())
	}
}

#[derive(Clone, Debug, Copy)]
pub enum SubscribeOrder {
	Ascending,
	Descending,
}

impl Decode for Option<SubscribeOrder> {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		match u64::decode(r)? {
			0 => Ok(None),
			1 => Ok(Some(SubscribeOrder::Ascending)),
			2 => Ok(Some(SubscribeOrder::Descending)),
			_ => Err(DecodeError::InvalidValue),
		}
	}
}

impl Encode for Option<SubscribeOrder> {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		let v: u64 = match self {
			None => 0,
			Some(SubscribeOrder::Ascending) => 1,
			Some(SubscribeOrder::Descending) => 2,
		};
		v.encode(w)
	}
}
