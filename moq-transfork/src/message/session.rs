use crate::coding::*;

#[derive(Clone, Debug)]
pub struct SessionInfo {
	pub bitrate: Option<u64>,
}

impl Decode for SessionInfo {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let bitrate = Option::<u64>::decode(r)?;
		Ok(Self { bitrate })
	}
}

impl Encode for SessionInfo {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.bitrate.encode(w);
	}
}
