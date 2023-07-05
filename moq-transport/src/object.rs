use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use bytes::{Buf, BufMut};

#[derive(Debug)]
pub struct Object {
	// An ID for this track.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/209
	pub track: VarInt,

	// The group sequence number.
	pub group: VarInt,

	// The object sequence number.
	pub sequence: VarInt,

	// The priority/send order.
	pub send_order: VarInt,
}

impl Decode for Object {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let typ = VarInt::decode(r)?;
		if typ.into_inner() != 0 {
			return Err(DecodeError::InvalidType(typ));
		}

		// NOTE: size has been omitted

		let track = VarInt::decode(r)?;
		let group = VarInt::decode(r)?;
		let sequence = VarInt::decode(r)?;
		let send_order = VarInt::decode(r)?;

		Ok(Self {
			track,
			group,
			sequence,
			send_order,
		})
	}
}

impl Encode for Object {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		VarInt::from_u32(0).encode(w)?;
		self.track.encode(w)?;
		self.group.encode(w)?;
		self.sequence.encode(w)?;
		self.send_order.encode(w)?;

		Ok(())
	}
}
