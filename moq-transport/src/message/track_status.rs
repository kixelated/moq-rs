use super::TrackStatusCode;
use crate::coding::{Decode, DecodeError, Encode, EncodeError};

#[derive(Clone, Debug)]
pub struct TrackStatus {
	/// Track Namespace
	pub track_namespace: String,
	/// Track Name
	pub track_name: String,
	/// Status Code
	pub status_code: TrackStatusCode,
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
			status_code: TrackStatusCode::decode(r)?,
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
