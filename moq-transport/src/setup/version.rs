use crate::coding::{Decode, DecodeError, Encode, EncodeError};

use crate::coding::{AsyncRead, AsyncWrite};

use std::ops::Deref;

/// A version number negotiated during the setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub u64);

impl Version {
	/// https://www.ietf.org/archive/id/draft-ietf-moq-transport-00.html
	pub const DRAFT_00: Version = Version(0xff000000);

	/// https://www.ietf.org/archive/id/draft-ietf-moq-transport-01.html
	pub const DRAFT_01: Version = Version(0xff000001);

	/// https://www.ietf.org/archive/id/draft-ietf-moq-transport-02.html
	pub const DRAFT_02: Version = Version(0xff000002);

	/// https://www.ietf.org/archive/id/draft-ietf-moq-transport-03.html
	pub const DRAFT_03: Version = Version(0xff000003);
}

impl From<u64> for Version {
	fn from(v: u64) -> Self {
		Self(v)
	}
}

impl From<Version> for u64 {
	fn from(v: Version) -> Self {
		v.0
	}
}

impl Version {
	/// Decode the version number.
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let v = u64::decode(r).await?;
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
		let count = u64::decode(r).await?;
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
		self.0.len().encode(w).await?;

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
