use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use bytes::{Buf, BufMut};

use std::time::Duration;

impl Encode for Duration {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		let ms = self.as_millis();
		let ms = VarInt::try_from(ms)?;
		ms.encode(w)
	}
}

impl Decode for Duration {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let ms = VarInt::decode(r)?;
		Ok(Self::from_millis(ms.into()))
	}
}
