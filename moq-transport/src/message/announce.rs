use crate::coding::{Decode, DecodeError, Encode, EncodeError, Params};

use crate::coding::{AsyncRead, AsyncWrite};
use crate::setup::Extensions;

/// Sent by the publisher to announce the availability of a group of tracks.
#[derive(Clone, Debug)]
pub struct Announce {
	/// The track namespace
	pub namespace: String,

	/// Optional parameters
	pub params: Params,
}

impl Announce {
	pub async fn decode<R: AsyncRead>(r: &mut R, _ext: &Extensions) -> Result<Self, DecodeError> {
		let namespace = String::decode(r).await?;
		let params = Params::decode(r).await?;

		Ok(Self { namespace, params })
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W, _ext: &Extensions) -> Result<(), EncodeError> {
		self.namespace.encode(w).await?;
		self.params.encode(w).await?;

		Ok(())
	}
}
