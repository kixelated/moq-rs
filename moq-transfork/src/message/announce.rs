use crate::coding::*;

/// Sent by the publisher to announce the availability of a group of tracks.
#[derive(Clone, Debug)]
pub struct Announce {
	/// The broadcast name
	pub broadcast: String,
}

impl Decode for Announce {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let broadcast = String::decode(r)?;
		Ok(Self { broadcast })
	}
}

impl Encode for Announce {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.broadcast.encode(w)?;
		Ok(())
	}
}

/// Sent by the subscriber to accept an Announce.
#[derive(Clone, Debug)]
pub struct AnnounceOk {}

impl Decode for AnnounceOk {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let cool = u64::decode(r)?;
		if cool != 1 {
			return Err(DecodeError::InvalidValue);
		}

		Ok(Self {})
	}
}

impl Encode for AnnounceOk {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		1u64.encode(w)?;
		Ok(())
	}
}
