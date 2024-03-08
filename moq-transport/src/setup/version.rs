use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use crate::coding::{AsyncRead, AsyncWrite};

use std::ops::Deref;

/// A version number negotiated during the setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub VarInt);

impl Version {
	/// https://www.ietf.org/archive/id/draft-ietf-moq-transport-00.html
	pub const DRAFT_00: Version = Version(VarInt::from_u32(0xff000000));

	/// https://www.ietf.org/archive/id/draft-ietf-moq-transport-01.html
	pub const DRAFT_01: Version = Version(VarInt::from_u32(0xff000001));

	/// https://www.ietf.org/archive/id/draft-ietf-moq-transport-02.html
	pub const DRAFT_02: Version = Version(VarInt::from_u32(0xff000002));

	/// https://www.ietf.org/archive/id/draft-ietf-moq-transport-03.html
	pub const DRAFT_03: Version = Version(VarInt::from_u32(0xff000003));
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

impl Version {
	/// Decode the version number.
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let v = VarInt::decode(r).await?;
		Ok(Self(v))
	}

	/// Encode the version number.
	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.0.encode(w).await?;
		Ok(())
	}
}

/// A list of versions in arbitrary order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Versions(Vec<Version>);

#[async_trait::async_trait]
impl Decode for Versions {
	/// Decode the version list.
	async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let count = VarInt::decode(r).await?.into_inner();
		let mut vs = Vec::new();

		for _ in 0..count {
			let v = Version::decode(r).await?;
			vs.push(v);
		}

		Ok(Self(vs))
	}
}

#[async_trait::async_trait]
impl Encode for Versions {
	/// Encode the version list.
	async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		let size: VarInt = self.0.len().try_into()?;
		size.encode(w).await?;

		for v in &self.0 {
			v.encode(w).await?;
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

impl<const N: usize> From<[Version; N]> for Versions {
	fn from(vs: [Version; N]) -> Self {
		Self(vs.to_vec())
	}
}
