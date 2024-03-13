use std::io::Cursor;
use std::{cmp::max, collections::HashMap};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::coding::{AsyncRead, AsyncWrite, Decode, Encode};

use crate::coding::{DecodeError, EncodeError};

#[derive(Default, Debug, Clone)]
pub struct Params(pub HashMap<u64, Vec<u8>>);

#[async_trait::async_trait]
impl Decode for Params {
	async fn decode<R: AsyncRead>(mut r: &mut R) -> Result<Self, DecodeError> {
		let mut params = HashMap::new();

		// I hate this encoding so much; let me encode my role and get on with my life.
		let count = u64::decode(r).await?;
		for _ in 0..count {
			let kind = u64::decode(r).await?;
			if params.contains_key(&kind) {
				return Err(DecodeError::DupliateParameter);
			}

			let size = u64::decode(r).await?;

			// Don't allocate the entire requested size to avoid a possible attack
			// Instead, we allocate up to 1024 and keep appending as we read further.
			let mut pr = r.take(size);
			let mut buf = Vec::with_capacity(max(1024, pr.limit() as usize));
			pr.read_to_end(&mut buf).await.map_err(|_| DecodeError::IoError)?;
			params.insert(kind, buf);

			r = pr.into_inner();
		}

		Ok(Params(params))
	}
}

#[async_trait::async_trait]
impl Encode for Params {
	async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.0.len().encode(w).await?;

		for (kind, value) in self.0.iter() {
			kind.encode(w).await?;
			value.len().encode(w).await?;
			w.write_all(value).await.map_err(|_| EncodeError::IoError)?;
		}

		Ok(())
	}
}

impl Params {
	pub fn new() -> Self {
		Self::default()
	}

	pub async fn set<P: Encode>(&mut self, kind: u64, p: P) -> Result<(), EncodeError> {
		let mut value = Vec::new();
		p.encode(&mut value).await?;
		self.0.insert(kind, value);

		Ok(())
	}

	pub fn has(&self, kind: u64) -> bool {
		self.0.contains_key(&kind)
	}

	pub async fn get<P: Decode>(&mut self, kind: u64) -> Result<Option<P>, DecodeError> {
		if let Some(value) = self.0.remove(&kind) {
			let mut cursor = Cursor::new(value);
			Ok(Some(P::decode(&mut cursor).await?))
		} else {
			Ok(None)
		}
	}
}
