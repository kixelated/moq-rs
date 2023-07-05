use super::{Role, Version};
use crate::coding::{Decode, DecodeError, Encode, EncodeError};

use bytes::{Buf, BufMut};

// Sent by the server in response to a client.
// NOTE: This is not a message type, but rather the control stream header.
// Proposal: https://github.com/moq-wg/moq-transport/issues/138
#[derive(Debug)]
pub struct SetupServer {
	// The list of supported versions in preferred order.
	pub version: Version,

	// param: 0x0: Indicate if the server is a publisher, a subscriber, or both.
	// Proposal: moq-wg/moq-transport#151
	pub role: Role,
}

impl Decode for SetupServer {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let version = Version::decode(r)?;
		let role = Role::decode(r)?;

		Ok(Self { version, role })
	}
}

impl Encode for SetupServer {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.version.encode(w)?;
		self.role.encode(w)?;

		Ok(())
	}
}
