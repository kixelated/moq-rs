use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::coding::*;

use super::Filter;

/// Send by the publisher, used to determine the message that follows.
#[derive(Clone, Copy, Debug, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
enum AnnounceStatus {
	Ended = 0,
	Active = 1,
	Live = 2,
}

/// Sent by the publisher to announce the availability of a track.
/// The payload contains the contents of the wildcard.
#[derive(Clone, Debug)]
pub enum Announce {
	Active(String),
	Ended(String),
	Live,
}

impl Decode for Announce {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(match AnnounceStatus::decode(r)? {
			AnnounceStatus::Active => Self::Active(String::decode(r)?),
			AnnounceStatus::Ended => Self::Ended(String::decode(r)?),
			AnnounceStatus::Live => Self::Live,
		})
	}
}

impl Encode for Announce {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		match self {
			Self::Active(capture) => {
				AnnounceStatus::Active.encode(w);
				capture.encode(w);
			}
			Self::Ended(capture) => {
				AnnounceStatus::Ended.encode(w);
				capture.encode(w);
			}
			Self::Live => AnnounceStatus::Live.encode(w),
		}
	}
}

/// Sent by the subscriber to request ANNOUNCE messages.
#[derive(Clone, Debug)]
pub struct AnnouncePlease {
	/// A wildcard filter.
	pub filter: Filter,
}

impl Decode for AnnouncePlease {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let filter = Filter::decode(r)?;
		Ok(Self { filter })
	}
}

impl Encode for AnnouncePlease {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.filter.encode(w)
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
