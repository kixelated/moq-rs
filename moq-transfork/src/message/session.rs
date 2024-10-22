use crate::coding::*;

#[derive(Clone, Debug)]
pub struct SessionInfo {
	pub bitrate: Option<u64>,
}

impl Decode for SessionInfo {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let bitrate = match u64::decode(r)? {
			0 => None,
			bitrate => Some(bitrate),
		};

		Ok(Self { bitrate })
	}
}

impl Encode for SessionInfo {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.bitrate.unwrap_or(0).encode(w);
	}
}
