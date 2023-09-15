use crate::coding::{AsyncRead, AsyncWrite};

use crate::coding::{DecodeError, EncodeError, VarInt};

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

impl From<Role> for VarInt {
	fn from(r: Role) -> Self {
		VarInt::from_u32(match r {
			Role::Publisher => 0x0,
			Role::Subscriber => 0x1,
			Role::Both => 0x2,
		})
	}
}

impl TryFrom<VarInt> for Role {
	type Error = DecodeError;

	fn try_from(v: VarInt) -> Result<Self, Self::Error> {
		match v.into_inner() {
			0x0 => Ok(Self::Publisher),
			0x1 => Ok(Self::Subscriber),
			0x2 => Ok(Self::Both),
			_ => Err(DecodeError::InvalidType(v)),
		}
	}
}

impl Role {
	/// Decode the role.
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let v = VarInt::decode(r).await?;
		v.try_into()
	}

	/// Encode the role.
	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		VarInt::from(*self).encode(w).await
	}
}
