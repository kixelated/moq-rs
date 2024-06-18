use crate::coding::{Decode, DecodeError, Encode, EncodeError};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ObjectStatus {
	Object = 0x0,
	ObjectDoesNotExist = 0x1,
	GroupDoesNotExist = 0x2,
	EndOfGroup = 0x3,
	EndOfTrack = 0x4,
}

impl Decode for ObjectStatus {
	fn decode<B: bytes::Buf>(r: &mut B) -> Result<Self, DecodeError> {
		match u64::decode(r)? {
			0x0 => Ok(Self::Object),
			0x1 => Ok(Self::ObjectDoesNotExist),
			0x2 => Ok(Self::GroupDoesNotExist),
			0x3 => Ok(Self::EndOfGroup),
			0x4 => Ok(Self::EndOfTrack),
			_ => Err(DecodeError::InvalidObjectStatus),
		}
	}
}

impl Encode for ObjectStatus {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		match self {
			Self::Object => (0x0_u64).encode(w),
			Self::ObjectDoesNotExist => (0x1_u64).encode(w),
			Self::GroupDoesNotExist => (0x2_u64).encode(w),
			Self::EndOfGroup => (0x3_u64).encode(w),
			Self::EndOfTrack => (0x4_u64).encode(w),
		}
	}
}

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

	// The object status
	pub object_status: ObjectStatus,
}

impl Decode for ObjectHeader {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			subscribe_id: u64::decode(r)?,
			track_alias: u64::decode(r)?,
			group_id: u64::decode(r)?,
			object_id: u64::decode(r)?,
			send_order: u64::decode(r)?,
			object_status: ObjectStatus::decode(r)?,
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
		self.object_status.encode(w)?;

		Ok(())
	}
}
