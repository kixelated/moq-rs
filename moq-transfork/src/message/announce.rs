use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::coding::*;
use crate::Path;

/// Send by the publisher, used to determine the message that follows.
#[derive(Clone, Copy, Debug, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
enum AnnounceStatus {
	Ended = 0,
	Active = 1,
	Live = 2,
}

/// Sent by the publisher to announce the availability of a track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Announce {
	Ended { suffix: Path },
	Active { suffix: Path },
	Live,
}

impl Decode for Announce {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(match AnnounceStatus::decode(r)? {
			AnnounceStatus::Ended => Self::Ended {
				suffix: Path::decode(r)?,
			},
			AnnounceStatus::Active => Self::Active {
				suffix: Path::decode(r)?,
			},
			AnnounceStatus::Live => Self::Live,
		})
	}
}

impl Encode for Announce {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		match self {
			Self::Ended { suffix } => {
				AnnounceStatus::Ended.encode(w);
				suffix.encode(w);
			}
			Self::Active { suffix } => {
				AnnounceStatus::Active.encode(w);
				suffix.encode(w);
			}
			Self::Live => AnnounceStatus::Live.encode(w),
		}
	}
}

/// Sent by the subscriber to request ANNOUNCE messages.
#[derive(Clone, Debug)]
pub struct AnnouncePlease {
	/// The desired track prefix
	pub prefix: Path,
}

impl Decode for AnnouncePlease {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let prefix = Path::decode(r)?;
		Ok(Self { prefix })
	}
}

impl Encode for AnnouncePlease {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.prefix.encode(w)
	}
}

impl Decode for AnnounceStatus {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let status = u8::decode(r)?;
		match status {
			0 => Ok(Self::Ended),
			1 => Ok(Self::Active),
			2 => Ok(Self::Live),
			_ => Err(DecodeError::InvalidValue),
		}
	}
}

impl Encode for AnnounceStatus {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		(*self as u8).encode(w)
	}
}
