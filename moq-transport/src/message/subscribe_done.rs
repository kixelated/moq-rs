use crate::coding::{Decode, DecodeError, Encode, EncodeError};

/// Sent by the publisher to cleanly terminate a Subscribe.
#[derive(Clone, Debug)]
pub struct SubscribeDone {
	/// The ID for this subscription.
	pub id: u64,

	/// The error code
	pub code: u64,

	/// An optional error reason
	pub reason: String,

	/// The final group/object sent on this subscription.
	pub last: Option<(u64, u64)>,
}

impl Decode for SubscribeDone {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let id = u64::decode(r)?;
		let code = u64::decode(r)?;
		let reason = String::decode(r)?;

		if r.remaining() < 1 {
			return Err(DecodeError::More(1));
		}

		let last = match r.get_u8() {
			0 => None,
			1 => Some((u64::decode(r)?, u64::decode(r)?)),
			_ => return Err(DecodeError::InvalidValue),
		};

		Ok(Self { id, code, reason, last })
	}
}

impl Encode for SubscribeDone {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w)?;
		self.code.encode(w)?;
		self.reason.encode(w)?;

		if w.remaining_mut() < 1 {
			return Err(EncodeError::More(1));
		}

		if let Some((group, object)) = self.last {
			w.put_u8(1);
			group.encode(w)?;
			object.encode(w)?;
		} else {
			w.put_u8(0);
		}

		Ok(())
	}
}
