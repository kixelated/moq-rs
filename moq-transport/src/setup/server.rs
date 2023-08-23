use super::{Role, Version};
use crate::{
	coding::{DecodeError, EncodeError},
	VarInt,
};

use webtransport_generic::{RecvStream, SendStream};

// Sent by the server in response to a client.
// NOTE: This is not a message type, but rather the control stream header.
// Proposal: https://github.com/moq-wg/moq-transport/issues/138
#[derive(Debug)]
pub struct Server {
	// The list of supported versions in preferred order.
	pub version: Version,

	// param: 0x0: Indicate if the server is a publisher, a subscriber, or both.
	// Proposal: moq-wg/moq-transport#151
	pub role: Role,
}

impl Server {
	pub async fn decode<R: RecvStream>(r: &mut R) -> Result<Self, DecodeError> {
		let typ = VarInt::decode(r).await?;
		if typ.into_inner() != 2 {
			return Err(DecodeError::InvalidType(typ));
		}

		let version = Version::decode(r).await?;
		let role = Role::decode(r).await?;

		Ok(Self { version, role })
	}

	pub async fn encode<W: SendStream>(&self, w: &mut W) -> Result<(), EncodeError> {
		VarInt::from_u32(2).encode(w).await?;
		self.version.encode(w).await?;
		self.role.encode(w).await?;

		Ok(())
	}
}
