use crate::coding::{Decode, DecodeError, Encode};

/// Sent by the subscriber to request all future objects for the given track.
///
/// Objects will use the provided ID instead of the full track name, to save bytes.
#[derive(Clone, Debug)]
pub struct Subscribe {
	pub id: u64,
	pub path: String,
	pub priority: i8,
}

impl Decode for Subscribe {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let id = u64::decode(r)?;
		let path = String::decode(r)?;
		let priority = i8::decode(r)?;

		Ok(Self { id, path, priority })
	}
}

impl Encode for Subscribe {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.id.encode(w);
		self.path.encode(w);
		self.priority.encode(w);
	}
}

#[derive(Clone, Debug)]
pub struct SubscribeUpdate {
	pub priority: u64,
}

impl Decode for SubscribeUpdate {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let priority = u64::decode(r)?;

		Ok(Self { priority })
	}
}

impl Encode for SubscribeUpdate {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.priority.encode(w);
	}
}

#[derive(Clone, Debug)]
pub struct SubscribeInfo {
	pub priority: i8,
	pub group: u64,
}

impl Encode for SubscribeInfo {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.priority.encode(w);
		self.group.encode(w);
	}
}

impl Decode for SubscribeInfo {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let priority = i8::decode(r)?;
		let group = u64::decode(r)?;

		Ok(Self { priority, group })
	}
}
