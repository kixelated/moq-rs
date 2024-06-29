use super::{Role, Versions};
use crate::coding::*;

/// Sent by the client to setup the session.
#[derive(Debug)]
pub struct Client {
	/// The list of supported versions in preferred order.
	pub versions: Versions,

	/// Indicate if the client is a publisher, a subscriber, or both.
	pub role: Role,

	pub path: Option<String>,

	/// Unknown parameters.
	pub unknown: Params,
}

impl Decode for Client {
	/// Decode a client setup message.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let versions = Versions::decode(r)?;
		let mut unknown = Params::decode(r)?;

		let role = unknown.remove::<Role>(0)?.ok_or(DecodeError::MissingParameter)?;
		let path = unknown.remove::<String>(1)?;

		Ok(Self {
			versions,
			role,
			path,
			unknown,
		})
	}
}

impl Encode for Client {
	/// Encode a server setup message.
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.versions.encode(w)?;

		let mut params = self.unknown.clone();
		params.insert(0, self.role)?;

		if let Some(path) = self.path.as_ref() {
			params.insert(1, path.as_str())?;
		}

		params.encode(w)?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::setup::Version;
	use bytes::BytesMut;

	#[test]
	fn client_coding() {
		let mut buf = BytesMut::new();
		let client = Client {
			versions: [Version::DRAFT_03].into(),
			role: Role::Both,
			path: None,
			unknown: Default::default(),
		};

		client.encode(&mut buf).unwrap();
		assert_eq!(
			buf.to_vec(),
			vec![0x01, 0xC0, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x03, 0x01, 0x00, 0x01, 0x03]
		);

		let decoded = Client::decode(&mut buf).unwrap();
		assert_eq!(decoded.versions, client.versions);
		assert_eq!(decoded.role, client.role);
		//assert_eq!(decoded.params, client.params);
	}
}
