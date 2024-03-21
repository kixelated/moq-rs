use crate::coding::{Decode, DecodeError, Encode, EncodeError};

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
	pub fn is_compatible(&self, other: Role) -> bool {
		self.is_publisher() == other.is_subscriber() && self.is_subscriber() == other.is_publisher()
	}
}

impl From<Role> for u64 {
	fn from(r: Role) -> Self {
		match r {
			Role::Publisher => 0x1,
			Role::Subscriber => 0x2,
			Role::Both => 0x3,
		}
	}
}

impl TryFrom<u64> for Role {
	type Error = DecodeError;

	fn try_from(v: u64) -> Result<Self, Self::Error> {
		match v {
			0x1 => Ok(Self::Publisher),
			0x2 => Ok(Self::Subscriber),
			0x3 => Ok(Self::Both),
			_ => Err(DecodeError::InvalidRole(v)),
		}
	}
}

impl Decode for Role {
	/// Decode the role.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let v = u64::decode(r)?;
		v.try_into()
	}
}

impl Encode for Role {
	/// Encode the role.
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		u64::from(*self).encode(w)
	}
}
