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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::setup::Role;
	use bytes::BytesMut;

	#[test]
	fn encode_decode() {
		let mut buf = BytesMut::new();
		let client = Server {
			version: Version::DRAFT_03,
			role: Role::Both,
			params: Params::default(),
		};

		client.encode(&mut buf).unwrap();
		assert_eq!(
			buf.to_vec(),
			vec![0x40, 0x41, 0xC0, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x03, 0x01, 0x00, 0x01, 0x03]
		);

		let decoded = Server::decode(&mut buf).unwrap();
		assert_eq!(decoded.version, client.version);
		assert_eq!(decoded.role, client.role);
	}
}
