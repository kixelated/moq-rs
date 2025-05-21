use std::sync::Arc;

use super::Sizer;

pub trait Encode: Sized {
	// Encode the value to the given writer.
	// This will panic if the Buf is not large enough; use a Vec or encode_size() to check.
	fn encode<W: bytes::BufMut>(&self, w: &mut W);

	// Return the size of the encoded value
	// Implementations can override this to provide a more efficient implementation
	fn encode_size(&self) -> usize {
		let mut sizer = Sizer::default();
		self.encode(&mut sizer);
		sizer.size
	}
}

impl Encode for u8 {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		w.put_u8(*self);
	}

	fn encode_size(&self) -> usize {
		1
	}
}

impl Encode for String {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.as_str().encode(w)
	}
}

impl Encode for &str {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.len().encode(w);
		w.put(self.as_bytes());
	}
}

impl Encode for std::time::Duration {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		let v: u64 = self.as_micros().try_into().expect("duration too large");
		v.encode(w);
	}
}

impl Encode for i8 {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		// This is not the usual way of encoding negative numbers.
		// i8 doesn't exist in the draft, but we use it instead of u8 for priority.
		// A default of 0 is more ergonomic for the user than a default of 128.
		w.put_u8(((*self as i16) + 128) as u8);
	}

	fn encode_size(&self) -> usize {
		1
	}
}

impl<T: Encode> Encode for &[T] {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.len().encode(w);
		for item in self.iter() {
			item.encode(w);
		}
	}
}

impl<T: Encode> Encode for Vec<T> {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.len().encode(w);
		for item in self.iter() {
			item.encode(w);
		}
	}
}

impl Encode for bytes::Bytes {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.len().encode(w);
		w.put_slice(self);
	}

	fn encode_size(&self) -> usize {
		self.len().encode_size() + self.len()
	}
}

impl<T: Encode> Encode for Arc<T> {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		(**self).encode(w);
	}

	fn encode_size(&self) -> usize {
		(**self).encode_size()
	}
}
