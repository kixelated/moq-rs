use crate::coding::{Decode, DecodeError, Encode, EncodeError};

#[derive(Clone, Debug)]
pub struct ObjectHeader {
	// The subscribe ID.
	pub subscribe_id: u64,

	// The track alias.
	pub track_alias: u64,

	// The sequence number within the track.
	pub group_id: u64,

	// The sequence number within the group.
	pub object_id: u64,

	// The send order, where **smaller** values are sent first.
	pub send_order: u64,
}

impl Decode for ObjectHeader {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			subscribe_id: u64::decode(r)?,
			track_alias: u64::decode(r)?,
			group_id: u64::decode(r)?,
			object_id: u64::decode(r)?,
			send_order: u64::decode(r)?,
		})
	}
}

impl Encode for ObjectHeader {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.subscribe_id.encode(w)?;
		self.track_alias.encode(w)?;
		self.group_id.encode(w)?;
		self.object_id.encode(w)?;
		self.send_order.encode(w)?;

		Ok(())
	}
}
