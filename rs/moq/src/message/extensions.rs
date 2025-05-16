use std::collections::HashMap;
use std::io::Cursor;

use crate::coding::*;

pub trait Extension: Encode + Decode {
	fn id() -> u64;
}

#[derive(Default, Debug, Clone)]
pub struct Extensions(HashMap<u64, Vec<u8>>);

impl Decode for Extensions {
	fn decode<R: bytes::Buf>(mut r: &mut R) -> Result<Self, DecodeError> {
		let mut map = HashMap::new();

		// I hate this encoding so much; let me encode my role and get on with my life.
		let count = u64::decode(r)?;
		for _ in 0..count {
			let kind = u64::decode(r)?;
			if map.contains_key(&kind) {
				return Err(DecodeError::DupliateParameter);
			}

			let data = Vec::<u8>::decode(&mut r)?;
			map.insert(kind, data);
		}

		Ok(Extensions(map))
	}
}

impl Encode for Extensions {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.0.len().encode(w);

		for (kind, value) in self.0.iter() {
			kind.encode(w);
			value.encode(w);
		}
	}
}

impl Extensions {
	pub fn get<E: Extension>(&self) -> Result<Option<E>, DecodeError> {
		Ok(match self.0.get(&E::id()) {
			Some(payload) => {
				let mut cursor = Cursor::new(payload);
				Some(E::decode(&mut cursor)?)
			}
			None => None,
		})
	}

	pub fn set<E: Extension>(&mut self, e: E) {
		let mut value = Vec::new();
		e.encode(&mut value);
		self.0.insert(E::id(), value);
	}
}
