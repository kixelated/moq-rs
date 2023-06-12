use bytes::{Buf, Bytes};
use std::str;

use super::VarInt;

pub trait Decode: Sized {
	fn decode<B: Buf>(buf: &mut B) -> anyhow::Result<Self>;
}

use thiserror::Error;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Error)]
#[error("unexpected end of buffer")]
pub struct UnexpectedEnd;

impl Decode for Bytes {
	fn decode<B: Buf>(buf: &mut B) -> anyhow::Result<Self> {
		let len = VarInt::decode(buf)?.into();
		anyhow::ensure!(buf.remaining() >= len, UnexpectedEnd);
		Ok(buf.copy_to_bytes(len))
	}
}

impl Decode for Vec<u8> {
	fn decode<B: Buf>(r: &mut B) -> anyhow::Result<Self> {
		let len = VarInt::decode(r)?.into();
		anyhow::ensure!(r.remaining() >= len, UnexpectedEnd);
		let v = r.copy_to_bytes(len).to_vec();
		Ok(v)
	}
}

impl<T: Decode> Decode for Vec<T> {
	fn decode<B: Buf>(r: &mut B) -> anyhow::Result<Self> {
		let len = VarInt::decode(r)?.into();

		let mut v = Vec::with_capacity(len);
		for _ in 0..len {
			v.push(T::decode(r)?);
		}

		Ok(v)
	}
}

impl Decode for String {
	fn decode<B: Buf>(r: &mut B) -> anyhow::Result<Self> {
		let data = Bytes::decode(r)?;
		let s = str::from_utf8(&data)?.to_string();
		Ok(s)
	}
}
