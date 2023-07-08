use super::{Role, Versions};
use crate::coding::{Decode, DecodeError, Encode, EncodeError};

use bytes::{Buf, BufMut};

// Sent by the client to setup up the session.
#[derive(Debug)]
pub struct SetupClient {
	// NOTE: This is not a message type, but rather the control stream header.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/138

	// The list of supported versions in preferred order.
	pub versions: Versions,

	// Indicate if the client is a publisher, a subscriber, or both.
	// Proposal: moq-wg/moq-transport#151
	pub role: Role,

	// The path, non-empty ONLY when not using WebTransport.
	pub path: String,
}

impl Decode for SetupClient {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let versions = Versions::decode(r)?;
		let role = Role::decode(r)?;
		let path = String::decode(r)?;

		Ok(Self { versions, role, path })
	}
}

impl Encode for SetupClient {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.versions.encode(w)?;
		self.role.encode(w)?;
		self.path.encode(w)?;

		Ok(())
	}
}
