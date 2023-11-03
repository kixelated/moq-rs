use super::{Extensions, Role, Versions};
use crate::{
	coding::{Decode, DecodeError, Encode, EncodeError, Params},
	VarInt,
};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the client to setup the session.
// NOTE: This is not a message type, but rather the control stream header.
// Proposal: https://github.com/moq-wg/moq-transport/issues/138
#[derive(Debug)]
pub struct Client {
	/// The list of supported versions in preferred order.
	pub versions: Versions,

	/// Indicate if the client is a publisher, a subscriber, or both.
	pub role: Role,

	/// A list of known/offered extensions.
	pub extensions: Extensions,

	/// Unknown parameters.
	pub params: Params,
}

impl Client {
	/// Decode a client setup message.
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let typ = VarInt::decode(r).await?;
		if typ.into_inner() != 0x40 {
			return Err(DecodeError::InvalidMessage(typ));
		}

		let versions = Versions::decode(r).await?;
		let mut params = Params::decode(r).await?;

		let role = params
			.get::<Role>(VarInt::from_u32(0))
			.await?
			.ok_or(DecodeError::MissingParameter)?;

		// Make sure the PATH parameter isn't used
		// TODO: This assumes WebTransport support only
		if params.has(VarInt::from_u32(1)) {
			return Err(DecodeError::InvalidParameter);
		}

		let extensions = Extensions::load(&mut params).await?;

		Ok(Self {
			versions,
			role,
			extensions,
			params,
		})
	}

	/// Encode a server setup message.
	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		VarInt::from_u32(0x40).encode(w).await?;
		self.versions.encode(w).await?;

		let mut params = self.params.clone();
		params.set(VarInt::from_u32(0), self.role).await?;
		self.extensions.store(&mut params).await?;

		params.encode(w).await?;

		Ok(())
	}
}
