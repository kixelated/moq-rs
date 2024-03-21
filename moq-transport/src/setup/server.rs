use super::{Role, Version};
use crate::coding::{Decode, DecodeError, Encode, EncodeError, Params};

/// Sent by the server in response to a client setup.
// NOTE: This is not a message type, but rather the control stream header.
// Proposal: https://github.com/moq-wg/moq-transport/issues/138
#[derive(Debug)]
pub struct Server {
	/// The list of supported versions in preferred order.
	pub version: Version,

	/// Indicate if the server is a publisher, a subscriber, or both.
	// Proposal: moq-wg/moq-transport#151
	pub role: Role,

	/// Unknown parameters.
	pub params: Params,
}

impl Decode for Server {
	/// Decode the server setup.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let typ = u64::decode(r)?;
		if typ != 0x41 {
			return Err(DecodeError::InvalidMessage(typ));
		}

		let version = Version::decode(r)?;
		let mut params = Params::decode(r)?;

		let role = params.get::<Role>(0)?.ok_or(DecodeError::MissingParameter)?;

		// Make sure the PATH parameter isn't used
		if params.has(1) {
			return Err(DecodeError::InvalidParameter);
		}

		Ok(Self { version, role, params })
	}
}

impl Encode for Server {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		0x41_u64.encode(w)?;
		self.version.encode(w)?;

		let mut params = self.params.clone();
		params.set(0, self.role)?;
		params.encode(w)?;

		Ok(())
	}
}
