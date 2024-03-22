use super::{Role, Versions};
use crate::coding::{Decode, DecodeError, Encode, EncodeError, Params};

/// Sent by the client to setup the session.
// NOTE: This is not a message type, but rather the control stream header.
// Proposal: https://github.com/moq-wg/moq-transport/issues/138
#[derive(Debug)]
pub struct Client {
	/// The list of supported versions in preferred order.
	pub versions: Versions,

	/// Indicate if the client is a publisher, a subscriber, or both.
	pub role: Role,

	/// Unknown parameters.
	pub params: Params,
}

impl Decode for Client {
	/// Decode a client setup message.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let typ = u64::decode(r)?;
		if typ != 0x40 {
			return Err(DecodeError::InvalidMessage(typ));
		}

		let versions = Versions::decode(r)?;
		let mut params = Params::decode(r)?;

		let role = params.get::<Role>(0)?.ok_or(DecodeError::MissingParameter)?;

		// Make sure the PATH parameter isn't used
		// TODO: This assumes WebTransport support only
		if params.has(1) {
			return Err(DecodeError::InvalidParameter);
		}

		Ok(Self { versions, role, params })
	}
}

impl Encode for Client {
	/// Encode a server setup message.
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		0x40_u64.encode(w)?;
		self.versions.encode(w)?;

		let mut params = self.params.clone();
		params.set(0, self.role)?;

		params.encode(w)?;

		Ok(())
	}
}
