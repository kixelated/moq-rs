use crate::coding::{Decode, DecodeError, Encode, EncodeError};

#[derive(Clone, Debug)]
pub struct TrackStatus {
	/// Track Namespace
	pub track_namespace: String,
	/// Track Name
	pub track_name: String,
	/// Status Code
	// TODO: encode/decode for values:
	// 0x00: The track is in progress, and subsequent fields contain the highest group and object ID for that track.
	// 0x01: The track does not exist. Subsequent fields MUST be zero, and any other value is a malformed message.
	// 0x02: The track has not yet begun. Subsequent fields MUST be zero. Any other value is a malformed message.
	// 0x03: The track has finished, so there is no "live edge." Subsequent fields contain the highest Group and object ID known.
	// 0x04: The sender is a relay that cannot obtain the current track status from upstream. Subsequent fields contain the largest group and object ID known.
	// And treat any other value as a malformed message.
	pub status_code: u64,
	/// Last Group ID
	pub last_group_id: u64,
	/// Last Object ID
	pub last_object_id: u64,
}

impl Decode for TrackStatus {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			track_namespace: String::decode(r)?,
			track_name: String::decode(r)?,
			status_code: u64::decode(r)?,
			last_group_id: u64::decode(r)?,
			last_object_id: u64::decode(r)?,
		})
	}
}

impl Encode for TrackStatus {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.track_namespace.encode(w)?;
		self.track_name.encode(w)?;
		self.status_code.encode(w)?;
		self.last_group_id.encode(w)?;
		self.last_object_id.encode(w)?;
		Ok(())
	}
}
