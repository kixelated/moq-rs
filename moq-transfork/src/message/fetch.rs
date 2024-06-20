use crate::coding::*;

pub struct Fetch {
	pub broadcast: String,
	pub track: String,
	pub priority: u64,
	pub group: u64,
	pub offset: usize,
}

impl Encode for Fetch {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.broadcast.encode(w)?;
		self.track.encode(w)?;
		self.priority.encode(w)?;
		self.group.encode(w)?;
		self.offset.encode(w)?;

		Ok(())
	}
}

impl Decode for Fetch {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let broadcast = String::decode(r)?;
		let track = String::decode(r)?;
		let priority = u64::decode(r)?;
		let group = u64::decode(r)?;
		let offset = usize::decode(r)?;

		Ok(Self {
			broadcast,
			track,
			priority,
			group,
			offset,
		})
	}
}
