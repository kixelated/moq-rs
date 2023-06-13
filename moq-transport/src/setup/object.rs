use crate::coding::{Decode, Encode, Size, VarInt};

#[derive(Default)]
pub struct Object {
	// An ID for this track.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/209
	pub track_id: VarInt,

	// The group sequence number.
	pub group_sequence: VarInt,

	// The object sequence number.
	pub object_sequence: VarInt,

	// The priority/send order.
	pub send_order: VarInt,
}

impl Decode for Object {
	fn decode<B: bytes::Buf>(r: &mut B) -> anyhow::Result<Self> {
		let track_id = VarInt::decode(r)?;
		let group_sequence = VarInt::decode(r)?;
		let object_sequence = VarInt::decode(r)?;
		let send_order = VarInt::decode(r)?;

		Ok(Self {
			track_id,
			group_sequence,
			object_sequence,
			send_order,
		})
	}
}

impl Encode for Object {
	fn encode<B: bytes::BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		self.track_id.encode(w)?;
		self.group_sequence.encode(w)?;
		self.object_sequence.encode(w)?;
		self.send_order.encode(w)?;

		Ok(())
	}
}

impl Size for Object {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(self.track_id.size()?
			+ self.group_sequence.size()?
			+ self.object_sequence.size()?
			+ self.send_order.size()?)
	}
}
