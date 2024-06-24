use std::time;

pub trait Encode: Sized {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError>;

	// Helper function to make sure we have enough bytes to encode
	fn encode_more<W: bytes::BufMut>(buf: &mut W, remain: usize) -> Result<(), EncodeError> {
		let needed = remain.saturating_sub(buf.remaining_mut());
		if needed > 0 {
			Err(EncodeError::More(needed))
		} else {
			Ok(())
		}
	}
}

/// An encode error.
#[derive(thiserror::Error, Debug, Clone)]
pub enum EncodeError {
	#[error("short buffer")]
	More(usize),

	#[error("value too large")]
	BoundsExceeded,

	#[error("invalid value")]
	InvalidValue,
}

impl Encode for u8 {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		w.put_u8(*self);
		Ok(())
	}
}

impl Encode for String {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.as_str().encode(w)
	}
}

impl Encode for &str {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.len().encode(w)?;
		Self::encode_more(w, self.len())?;
		w.put(self.as_bytes());
		Ok(())
	}
}

impl Encode for Vec<u8> {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.len().encode(w)?;
		Self::encode_more(w, self.len())?;
		w.put_slice(self);
		Ok(())
	}
}

impl Encode for Option<u64> {
	/// Encode a varint to the given writer.
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.map(|v| v + 1).unwrap_or(0).encode(w)
	}
}

impl Encode for Option<usize> {
	/// Encode a varint to the given writer.
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.map(|v| v + 1).unwrap_or(0).encode(w)
	}
}

impl Encode for time::Duration {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		let v: u64 = self.as_millis().try_into().map_err(|_| EncodeError::BoundsExceeded)?;
		v.encode(w)
	}
}

impl Encode for Option<time::Duration> {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		let v: u64 = match self {
			None => 0,
			Some(v) => (v.as_millis() + 1)
				.try_into()
				.map_err(|_| EncodeError::BoundsExceeded)?,
		};

		v.encode(w)
	}
}
