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
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.versions.encode(w);
		self.extensions.encode(w);
	}
}
