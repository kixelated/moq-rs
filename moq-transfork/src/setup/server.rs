use super::{Extensions, Version};
use crate::coding::*;

/// Sent by the server in response to a client setup.
#[derive(Debug, Clone)]
pub struct Server {
	/// The list of supported versions in preferred order.
	pub version: Version,

	/// Supported extenisions.
	pub extensions: Extensions,
}

impl Decode for Server {
	/// Decode the server setup.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let version = Version::decode(r)?;
		let extensions = Extensions::decode(r)?;

		Ok(Self { version, extensions })
	}
}

impl Encode for Server {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.version.encode(w)?;
		self.extensions.encode(w)?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::setup::{Role, Version};
	use bytes::BytesMut;

	#[test]
	fn server_coding() {
		let mut buf = BytesMut::new();
		let mut extensions = Extensions::default();
		extensions.set(Role::Both).unwrap();

		let client = Server {
			version: Version::DRAFT_03,
			extensions,
		};

		client.encode(&mut buf).unwrap();
		assert_eq!(
			buf.to_vec(),
			vec![0xC0, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x03, 0x01, 0x00, 0x01, 0x03]
		);

		let decoded = Server::decode(&mut buf).unwrap();
		assert_eq!(decoded.version, client.version);

		let role = decoded
			.extensions
			.get()
			.expect("missing extension")
			.expect("failed to decode role");
		assert_eq!(Role::Both, role);
	}
}
