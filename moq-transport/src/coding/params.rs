use std::collections::HashMap;
use std::io::Cursor;

use crate::coding::{Decode, DecodeError, Encode, EncodeError};

#[derive(Default, Debug, Clone)]
pub struct Params(pub HashMap<u64, Vec<u8>>);

impl Decode for Params {
	fn decode<R: bytes::Buf>(mut r: &mut R) -> Result<Self, DecodeError> {
		let mut params = HashMap::new();

		// I hate this encoding so much; let me encode my role and get on with my life.
		let count = u64::decode(r)?;
		for _ in 0..count {
			let kind = u64::decode(r)?;
			if params.contains_key(&kind) {
				return Err(DecodeError::DupliateParameter);
			}

			let size = usize::decode(&mut r)?;

			if r.remaining() < size {
				return Err(DecodeError::More(size));
			}

			// Don't allocate the entire requested size to avoid a possible attack
			// Instead, we allocate up to 1024 and keep appending as we read further.
			let mut buf = vec![0; size];
			r.copy_to_slice(&mut buf);

			params.insert(kind, buf);
		}

		Ok(Params(params))
	}
}

impl Encode for Params {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.0.len().encode(w)?;

		for (kind, value) in self.0.iter() {
			kind.encode(w)?;
			value.len().encode(w)?;

			if w.remaining_mut() < value.len() {
				return Err(EncodeError::More(value.len()));
			}

			w.put_slice(value);
		}

		Ok(())
	}
}

impl Params {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn set<P: Encode>(&mut self, kind: u64, p: P) -> Result<(), EncodeError> {
		let mut value = Vec::new();
		p.encode(&mut value)?;
		self.0.insert(kind, value);

		Ok(())
	}

	pub fn has(&self, kind: u64) -> bool {
		self.0.contains_key(&kind)
	}

	pub fn get<P: Decode>(&mut self, kind: u64) -> Result<Option<P>, DecodeError> {
		if let Some(value) = self.0.remove(&kind) {
			let mut cursor = Cursor::new(value);
			Ok(Some(P::decode(&mut cursor)?))
		} else {
			Ok(None)
		}
	}
}
