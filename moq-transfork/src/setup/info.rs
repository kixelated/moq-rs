use crate::coding::*;

#[derive(Clone, Debug)]
pub struct Info {
	pub bitrate: Option<u64>,
}

impl Decode for Info {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let bitrate = Option::<u64>::decode(r)?;
		Ok(Self { bitrate })
	}
}

impl Encode for Info {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.bitrate.encode(w)?;
		Ok(())
	}
}
