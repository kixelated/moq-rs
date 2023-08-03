use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use std::time::Duration;

use bytes::{Buf, BufMut};

#[derive(Debug)]
pub struct SubscribeOk {
	// NOTE: No full track name because of this proposal: https://github.com/moq-wg/moq-transport/issues/209

	// The ID for this track.
	pub track_id: VarInt,

	// The subscription will end after this duration has elapsed.
	// A value of zero is invalid.
	pub expires: Option<Duration>,
}

impl Decode for SubscribeOk {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let track_id = VarInt::decode(r)?;
		let expires = Duration::decode(r)?;
		let expires = if expires == Duration::ZERO { None } else { Some(expires) };

		Ok(Self { track_id, expires })
	}
}

impl Encode for SubscribeOk {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.track_id.encode(w)?;
		self.expires.unwrap_or_default().encode(w)?;

		Ok(())
	}
}
