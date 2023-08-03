use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use bytes::{Buf, BufMut};

use std::ops::Deref;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub VarInt);

impl Version {
	pub const DRAFT_00: Version = Version(VarInt::from_u32(0xff00));
}

impl From<VarInt> for Version {
	fn from(v: VarInt) -> Self {
		Self(v)
	}
}

impl From<Version> for VarInt {
	fn from(v: Version) -> Self {
		v.0
	}
}

impl Decode for Version {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let v = VarInt::decode(r)?;
		Ok(Self(v))
	}
}

impl Encode for Version {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.0.encode(w)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Versions(pub Vec<Version>);

impl Decode for Versions {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let count = VarInt::decode(r)?.into_inner();
		let mut vs = Vec::new();

		for _ in 0..count {
			let v = Version::decode(r)?;
			vs.push(v);
		}

		Ok(Self(vs))
	}
}

impl Encode for Versions {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		let size: VarInt = self.0.len().try_into()?;
		size.encode(w)?;

		for v in &self.0 {
			v.encode(w)?;
		}

		Ok(())
	}
}

impl Deref for Versions {
	type Target = Vec<Version>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl From<Vec<Version>> for Versions {
	fn from(vs: Vec<Version>) -> Self {
		Self(vs)
	}
}
