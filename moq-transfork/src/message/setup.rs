use super::{Extensions, Version, Versions};
use crate::coding::*;

/// Sent by the client to setup the session.
#[derive(Debug, Clone)]
pub struct ClientSetup {
	/// The list of supported versions in preferred order.
	pub versions: Versions,

	/// Extensions.
	pub extensions: Extensions,
}

impl Decode for ClientSetup {
	/// Decode a client setup message.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let versions = Versions::decode(r)?;
		let extensions = Extensions::decode(r)?;

		Ok(Self { versions, extensions })
	}
}

impl Encode for ClientSetup {
	/// Encode a server setup message.
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.versions.encode(w);
		self.extensions.encode(w);
	}
}

/// Sent by the server in response to a client setup.
#[derive(Debug, Clone)]
pub struct ServerSetup {
	/// The list of supported versions in preferred order.
	pub version: Version,

	/// Supported extenisions.
	pub extensions: Extensions,
}

impl Decode for ServerSetup {
	/// Decode the server setup.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let version = Version::decode(r)?;
		let extensions = Extensions::decode(r)?;

		Ok(Self { version, extensions })
	}
}

impl Encode for ServerSetup {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.version.encode(w);
		self.extensions.encode(w);
	}
}
