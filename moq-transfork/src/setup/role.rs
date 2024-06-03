use crate::coding::*;

/// Indicates the endpoint is a publisher, subscriber, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
	Publisher,
	Subscriber,
	Both,
}

impl Role {
	/// Returns true if the role is publisher.
	pub fn is_publisher(&self) -> bool {
		match self {
			Self::Publisher | Self::Both => true,
			Self::Subscriber => false,
		}
	}

	/// Returns true if the role is a subscriber.
	pub fn is_subscriber(&self) -> bool {
		match self {
			Self::Subscriber | Self::Both => true,
			Self::Publisher => false,
		}
	}

	/// Returns true if two endpoints are compatible.
	pub fn is_compatible(&self, other: Self) -> bool {
		self.is_publisher() == other.is_subscriber() && self.is_subscriber() == other.is_publisher()
	}
}

impl Decode for Role {
	/// Decode the role.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let v = u64::decode(r)?;
		match v {
			0x1 => Ok(Self::Publisher),
			0x2 => Ok(Self::Subscriber),
			0x3 => Ok(Self::Both),
			_ => Err(DecodeError::InvalidRole(v)),
		}
	}
}

impl Encode for Role {
	/// Encode the role.
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		match self {
			Self::Publisher => 0u64,
			Self::Subscriber => 1u64,
			Self::Both => 2u64,
		}
		.encode(w)
	}
}
