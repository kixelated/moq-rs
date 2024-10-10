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
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.broadcast.encode(w)
	}
}

/// Sent by the subscriber to request ANNOUNCE messages.
#[derive(Clone, Debug)]
pub struct AnnounceInterest {
	/// The desired broadcast prefix
	pub prefix: String,
}

impl Decode for AnnounceInterest {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let prefix = String::decode(r)?;
		Ok(Self { prefix })
	}
}

impl Encode for AnnounceInterest {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.prefix.encode(w)
	}
}
