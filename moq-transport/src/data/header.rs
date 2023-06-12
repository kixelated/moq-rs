use crate::coding::{Decode, Encode, Size, VarInt};

// NOTE: This is an OBJECT in the moq-transport draft.
#[derive(Default)]
pub struct Header {
	// An ID for this track.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/209
	track_id: VarInt,

	// The group sequence number.
	group_sequence: VarInt,

	// The object sequence number.
	object_sequence: VarInt,

	// The priority/send order.
	send_order: VarInt,
}

impl Decode for Header {
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

impl Encode for Header {
	fn encode<B: bytes::BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		self.track_id.encode(w)?;
		self.group_sequence.encode(w)?;
		self.object_sequence.encode(w)?;
		self.send_order.encode(w)?;

		Ok(())
	}
}

impl Size for Header {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(self.track_id.size()?
			+ self.group_sequence.size()?
			+ self.object_sequence.size()?
			+ self.send_order.size()?)
	}
}
