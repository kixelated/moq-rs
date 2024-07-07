use crate::coding::*;

use super::Extension;

/// Indicates the endpoint is a publisher, subscriber, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
	Publisher,
	Subscriber,
	Both,
	Any,
}

impl Default for Role {
	fn default() -> Self {
		Self::Both
	}
}

impl Role {
	/// Returns true if the role is publisher.
	pub fn is_publisher(&self) -> bool {
		*self != Self::Subscriber
	}

	/// Returns true if the role is a subscriber.
	pub fn is_subscriber(&self) -> bool {
		*self != Self::Publisher
	}

	// Given the peer's role, downgrade the role to the most compatible role.
	pub fn downgrade(&self, other: Self) -> Option<Self> {
		match self {
			Self::Both => match other {
				Self::Both | Self::Any => Some(Self::Both),
				_ => None,
			},
			Self::Publisher => match other {
				Self::Subscriber | Self::Any => Some(Self::Publisher),
				_ => None,
			},
			Self::Subscriber => match other {
				Self::Publisher | Self::Any => Some(Self::Subscriber),
				_ => None,
			},
			Self::Any => match other {
				Self::Both => Some(Self::Both),
				Self::Publisher => Some(Self::Subscriber),
				Self::Subscriber => Some(Self::Publisher),
				Self::Any => Some(Self::Any),
			},
		}
	}
}

impl Decode for Role {
	/// Decode the role.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let v = u64::decode(r)?;
		match v {
			1u64 => Ok(Self::Publisher),
			2u64 => Ok(Self::Subscriber),
			3u64 => Ok(Self::Both),
			4u64 => Ok(Self::Any),
			_ => Err(DecodeError::InvalidRole(v)),
		}
	}
}

impl Encode for Role {
	/// Encode the role.
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		match self {
			Self::Publisher => 1u64,
			Self::Subscriber => 2u64,
			Self::Both => 3u64,
			Self::Any => 4u64,
		}
		.encode(w)
	}
}

impl Extension for Role {
	fn id() -> u64 {
		0x00
	}
}
