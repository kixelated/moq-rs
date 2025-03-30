use crate::{
	coding::{Decode, DecodeError, Encode},
	message::GroupOrder,
};

/// Sent by the subscriber to request all future objects for the given track.
///
/// Objects will use the provided ID instead of the full track name, to save bytes.
#[derive(Clone, Debug)]
pub struct Subscribe {
	pub id: u64,
	pub path: String,
	pub priority: i8,
	pub order: GroupOrder,

	// TODO remove
	pub start: Option<u64>,
	pub end: Option<u64>,
}

impl Decode for Subscribe {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let id = u64::decode(r)?;
		let path = String::decode(r)?;
		let priority = i8::decode(r)?;
		let order = GroupOrder::decode(r)?;
		let start = match u64::decode(r)? {
			0 => None,
			n => Some(n - 1),
		};
		let end = match u64::decode(r)? {
			0 => None,
			n => Some(n - 1),
		};

		Ok(Self {
			id,
			path,
			priority,
			order,
			start,
			end,
		})
	}
}

impl Encode for Subscribe {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.id.encode(w);
		self.path.encode(w);
		self.priority.encode(w);
		self.order.encode(w);
		self.start.map(|v| v + 1).unwrap_or(0).encode(w);
		self.end.map(|v| v + 1).unwrap_or(0).encode(w);
	}
}

#[derive(Clone, Debug)]
pub struct SubscribeUpdate {
	pub priority: i8,
	pub order: GroupOrder,

	// TODO remove
	pub start: Option<u64>,
	pub end: Option<u64>,
}

impl Decode for SubscribeUpdate {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let priority = i8::decode(r)?;
		let order = GroupOrder::decode(r)?;
		let start = match u64::decode(r)? {
			0 => None,
			n => Some(n - 1),
		};
		let end = match u64::decode(r)? {
			0 => None,
			n => Some(n - 1),
		};

		Ok(Self {
			priority,
			order,
			start,
			end,
		})
	}
}

impl Encode for SubscribeUpdate {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.priority.encode(w);
		self.order.encode(w);
		self.start.map(|v| v + 1).unwrap_or(0).encode(w);
		self.end.map(|v| v + 1).unwrap_or(0).encode(w);
	}
}
