use crate::coding::{AsyncRead, AsyncWrite};

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

#[async_trait::async_trait]
impl Decode for Role {
	/// Decode the role.
	async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let v = u64::decode(r).await?;
		v.try_into()
	}
}

#[async_trait::async_trait]
impl Encode for Role {
	/// Encode the role.
	async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		u64::from(*self).encode(w).await
	}
}
