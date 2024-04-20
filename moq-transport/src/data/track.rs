use crate::coding::{Decode, DecodeError, Encode, EncodeError};

#[derive(Clone, Debug)]
pub struct TrackHeader {
	// The subscribe ID.
	pub subscribe_id: u64,

	// The track ID.
	pub track_alias: u64,

	// The priority, where **smaller** values are sent first.
	pub send_order: u64,
}

impl Decode for TrackHeader {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			subscribe_id: u64::decode(r)?,
			track_alias: u64::decode(r)?,
			send_order: u64::decode(r)?,
		})
	}
}

impl Encode for TrackHeader {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.subscribe_id.encode(w)?;
		self.track_alias.encode(w)?;
		self.send_order.encode(w)?;

		Ok(())
	}
}

#[derive(Clone, Debug)]
pub struct TrackObject {
	pub group_id: u64,
	pub object_id: u64,
	pub size: usize,
}

impl Decode for TrackObject {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let group_id = u64::decode(r)?;

		let object_id = u64::decode(r)?;
		let size = usize::decode(r)?;

		Ok(Self {
			group_id,
			object_id,
			size,
		})
	}
}

impl Encode for TrackObject {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.group_id.encode(w)?;
		self.object_id.encode(w)?;
		self.size.encode(w)?;

		Ok(())
	}
}
