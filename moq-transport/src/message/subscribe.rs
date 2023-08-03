use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use bytes::{Buf, BufMut};

#[derive(Debug)]
pub struct Subscribe {
	// An ID we choose so we can map to the track_name.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/209
	pub track_id: VarInt,

	// The track namespace.
	pub track_namespace: String,

	// The track name.
	pub track_name: String,
}

impl Decode for Subscribe {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let track_id = VarInt::decode(r)?;
		let track_namespace = String::decode(r)?;
		let track_name = String::decode(r)?;

		Ok(Self {
			track_id,
			track_namespace,
			track_name,
		})
	}
}

impl Encode for Subscribe {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.track_id.encode(w)?;
		self.track_namespace.encode(w)?;
		self.track_name.encode(w)?;

		Ok(())
	}
}
