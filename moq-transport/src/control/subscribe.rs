use crate::coding::{Decode, Encode, Param, Params, Size, VarInt};

use bytes::{Buf, BufMut, Bytes};

#[derive(Default)]
pub struct Subscribe {
	// An ID we choose so we can map to the track_name.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/209
	pub track_id: VarInt,

	// The track namespace + track name.
	pub track_name: String,

	// The group sequence number, param 0x00
	pub group_sequence: Param<0, VarInt>,

	// The object sequence number, param 0x01
	pub object_sequence: Param<1, VarInt>,

	// An authentication token, param 0x02
	pub auth: Param<2, Bytes>,

	// Parameters that we don't recognize.
	pub unknown: Params,
}

impl Decode for Subscribe {
	fn decode<B: Buf>(r: &mut B) -> anyhow::Result<Self> {
		let track_id = VarInt::decode(r)?;
		let track_name = String::decode(r)?;

		let mut group_sequence = Param::new();
		let mut object_sequence = Param::new();
		let mut auth = Param::new();
		let mut unknown = Params::new();

		while r.has_remaining() {
			// TODO is there some way to peek at this varint? I would like to enforce the correct ID in decode.
			let id = VarInt::decode(r)?;

			match u64::from(id) {
				0 => group_sequence = Param::decode(r)?,
				1 => object_sequence = Param::decode(r)?,
				2 => auth = Param::decode(r)?,
				_ => unknown.decode_param(r)?,
			}
		}

		Ok(Self {
			track_id,
			track_name,
			group_sequence,
			object_sequence,
			auth,
			unknown,
		})
	}
}

impl Encode for Subscribe {
	fn encode<B: BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		self.track_id.encode(w)?;
		self.track_name.encode(w)?;

		self.group_sequence.encode(w)?;
		self.object_sequence.encode(w)?;
		self.auth.encode(w)?;
		self.unknown.encode(w)?;

		Ok(())
	}
}

impl Size for Subscribe {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(self.track_id.size()?
			+ self.track_name.size()?
			+ self.group_sequence.size()?
			+ self.object_sequence.size()?
			+ self.auth.size()?
			+ self.unknown.size()?)
	}
}
