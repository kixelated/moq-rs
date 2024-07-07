use super::{Extensions, Versions};
use crate::coding::*;

/// Sent by the client to setup the session.
#[derive(Debug, Clone)]
pub struct Client {
	/// The list of supported versions in preferred order.
	pub versions: Versions,

	/// Extensions.
	pub extensions: Extensions,
}

impl Decode for Client {
	/// Decode a client setup message.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let versions = Versions::decode(r)?;
		let extensions = Extensions::decode(r)?;

		Ok(Self { versions, extensions })
	}
}

impl Encode for Client {
	/// Encode a server setup message.
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.versions.encode(w)?;
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
	fn client_coding() {
		let mut buf = BytesMut::new();

		let mut extensions = Extensions::default();
		extensions.set(Role::Both).unwrap();

		let client = Client {
			versions: [Version::DRAFT_03].into(),
			extensions,
		};

		client.encode(&mut buf).unwrap();
		assert_eq!(
			buf.to_vec(),
			vec![0x01, 0xC0, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x03, 0x01, 0x00, 0x01, 0x03]
		);

		let decoded = Client::decode(&mut buf).unwrap();
		assert_eq!(decoded.versions, client.versions);

		let role = decoded
			.extensions
			.get::<Role>()
			.expect("no extension found")
			.expect("failed to decode");
		assert_eq!(Role::Both, role);
	}
}
