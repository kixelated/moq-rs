use std::collections::HashMap;
use std::io::Cursor;

use super::{Decode, DecodeError, Encode, EncodeError};

#[derive(Default, Debug, Clone)]
pub struct Params(HashMap<u64, Vec<u8>>);

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

			let data = Vec::<u8>::decode(&mut r)?;
			params.insert(kind, data);
		}

		Ok(Params(params))
	}
}

impl Encode for Params {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.0.len().encode(w)?;

		for (kind, value) in self.0.iter() {
			kind.encode(w)?;
			value.encode(w)?;
		}

		Ok(())
	}
}

impl Params {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn insert<P: Encode>(&mut self, kind: u64, p: P) -> Result<(), EncodeError> {
		let mut value = Vec::new();
		p.encode(&mut value)?;
		self.0.insert(kind, value);

		Ok(())
	}

	pub fn has(&self, kind: u64) -> bool {
		self.0.contains_key(&kind)
	}

	pub fn remove<P: Decode>(&mut self, kind: u64) -> Result<Option<P>, DecodeError> {
		if let Some(value) = self.0.remove(&kind) {
			let mut cursor = Cursor::new(value);
			Ok(Some(P::decode(&mut cursor)?))
		} else {
			Ok(None)
		}
	}
}
