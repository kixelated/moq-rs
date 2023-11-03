use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use crate::coding::{AsyncRead, AsyncWrite};
use crate::setup::Extensions;

/// Sent by the subscriber to terminate a Subscribe.
#[derive(Clone, Debug)]
pub struct Unsubscribe {
	// NOTE: No full track name because of this proposal: https://github.com/moq-wg/moq-transport/issues/209

	// The ID for this subscription.
	pub id: VarInt,
}

impl Unsubscribe {
	pub async fn decode<R: AsyncRead>(r: &mut R, _ext: &Extensions) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		Ok(Self { id })
	}
}

impl Unsubscribe {
	pub async fn encode<W: AsyncWrite>(&self, w: &mut W, _ext: &Extensions) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		Ok(())
	}
}
