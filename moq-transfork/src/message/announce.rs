use crate::coding::*;
use crate::Path;

/// Sent by the publisher to announce the availability of a group of tracks.
#[derive(Clone, Debug)]
pub struct Announce {
	/// The broadcast status, either active or ended
	pub status: AnnounceStatus,

	/// The path suffix
	pub suffix: Path,
}

impl Decode for Announce {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let status = AnnounceStatus::decode(r)?;
		let suffix = Path::decode(r)?;
		Ok(Self { status, suffix })
	}
}

impl Encode for Announce {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.status.encode(w);
		self.suffix.encode(w)
	}
}

/// Sent by the subscriber to request ANNOUNCE messages.
#[derive(Clone, Debug)]
pub struct AnnounceInterest {
	/// The desired broadcast prefix
	pub prefix: Path,
}

impl Decode for AnnounceInterest {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let prefix = Path::decode(r)?;
		Ok(Self { prefix })
	}
}

impl Encode for AnnounceInterest {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.prefix.encode(w)
	}
}

#[derive(Clone, Debug)]
pub enum AnnounceStatus {
	Active,
	Ended,
}

impl Decode for AnnounceStatus {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let status = u8::decode(r)?;
		match status {
			0 => Ok(Self::Active),
			1 => Ok(Self::Ended),
			_ => Err(DecodeError::InvalidValue),
		}
	}
}

impl Encode for AnnounceStatus {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		match self {
			Self::Active => 0u8.encode(w),
			Self::Ended => 1u8.encode(w),
		}
	}
}
