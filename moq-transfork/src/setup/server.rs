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
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.version.encode(w);
		self.extensions.encode(w);
	}
}
