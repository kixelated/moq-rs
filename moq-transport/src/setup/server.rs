use super::{Extensions, Role, Version};
use crate::{
	coding::{Decode, DecodeError, Encode, EncodeError, Params},
	VarInt,
};

use crate::coding::{AsyncRead, AsyncWrite};

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

	/// Custom extensions.
	pub extensions: Extensions,

	/// Unknown parameters.
	pub params: Params,
}

impl Server {
	/// Decode the server setup.
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let typ = VarInt::decode(r).await?;
		if typ.into_inner() != 0x41 {
			return Err(DecodeError::InvalidMessage(typ));
		}

		let version = Version::decode(r).await?;
		let mut params = Params::decode(r).await?;

		let role = params
			.get::<Role>(VarInt::from_u32(0))
			.await?
			.ok_or(DecodeError::MissingParameter)?;

		// Make sure the PATH parameter isn't used
		if params.has(VarInt::from_u32(1)) {
			return Err(DecodeError::InvalidParameter);
		}

		let extensions = Extensions::load(&mut params).await?;

		Ok(Self {
			version,
			role,
			extensions,
			params,
		})
	}

	/// Encode the server setup.
	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		VarInt::from_u32(0x41).encode(w).await?;
		self.version.encode(w).await?;

		let mut params = self.params.clone();
		params.set(VarInt::from_u32(0), self.role).await?;
		self.extensions.store(&mut params).await?;
		params.encode(w).await?;

		Ok(())
	}
}
