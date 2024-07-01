use super::{Role, Version};
use crate::coding::*;

/// Sent by the server in response to a client setup.
pub struct Server {
	/// The list of supported versions in preferred order.
	pub version: Version,

	/// Indicate if the server is a publisher, a subscriber, or both.
	pub role: Role,

	/// Unknown parameters.
	pub unknown: Params,
}

impl Decode for Server {
	/// Decode the server setup.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let version = Version::decode(r)?;
		let mut params = Params::decode(r)?;

		let role = params.remove::<Role>(0)?.ok_or(DecodeError::MissingParameter)?;

		// Make sure the PATH parameter isn't used
		if params.has(1) {
			return Err(DecodeError::InvalidParameter);
		}

		Ok(Self {
			version,
			role,
			unknown: params,
		})
	}
}

impl Encode for Server {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.version.encode(w)?;

		let mut params = self.unknown.clone();
		params.insert(0, self.role)?;
		params.encode(w)?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::BytesMut;

	#[test]
	fn server_coding() {
		let mut buf = BytesMut::new();
		let client = Server {
			version: Version::DRAFT_03,
			role: Role::Both,
			unknown: Default::default(),
		};

		client.encode(&mut buf).unwrap();
		assert_eq!(
			buf.to_vec(),
			vec![0xC0, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x03, 0x01, 0x00, 0x01, 0x03]
		);

		let decoded = Server::decode(&mut buf).unwrap();
		assert_eq!(decoded.version, client.version);
		assert_eq!(decoded.role, client.role);
	}
}
