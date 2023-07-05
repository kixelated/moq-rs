use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use bytes::{Buf, BufMut};

#[derive(Debug)]
pub struct AnnounceError {
	// Echo back the namespace that was announced.
	// TODO Propose using an ID to save bytes.
	pub track_namespace: String,

	// An error code.
	pub code: VarInt,

	// An optional, human-readable reason.
	pub reason: String,
}

impl Decode for AnnounceError {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let track_namespace = String::decode(r)?;
		let code = VarInt::decode(r)?;
		let reason = String::decode(r)?;

		Ok(Self {
			track_namespace,
			code,
			reason,
		})
	}
}

impl Encode for AnnounceError {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.track_namespace.encode(w)?;
		self.code.encode(w)?;
		self.reason.encode(w)?;

		Ok(())
	}
}
