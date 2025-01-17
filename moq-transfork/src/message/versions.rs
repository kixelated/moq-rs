use crate::coding::*;

use std::{fmt, ops::Deref};

/// A version number negotiated during the setup.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(u64);

impl Version {
	/// <https://www.ietf.org/archive/id/draft-ietf-moq-transport-00.html>
	pub const DRAFT_00: Version = Version(0xff000000);

	/// <https://www.ietf.org/archive/id/draft-ietf-moq-transport-01.html>
	pub const DRAFT_01: Version = Version(0xff000001);

	/// <https://www.ietf.org/archive/id/draft-ietf-moq-transport-02.html>
	pub const DRAFT_02: Version = Version(0xff000002);

	/// <https://www.ietf.org/archive/id/draft-ietf-moq-transport-03.html>
	pub const DRAFT_03: Version = Version(0xff000003);

	/// <https://www.ietf.org/archive/id/draft-ietf-moq-transport-04.html>
	pub const DRAFT_04: Version = Version(0xff000004);

	/// <https://www.ietf.org/archive/id/draft-lcurley-moq-transfork-00.html>
	pub const FORK_00: Version = Version(0xff0bad00);

	/// <https://www.ietf.org/archive/id/draft-lcurley-moq-transfork-01.html>
	pub const FORK_01: Version = Version(0xff0bad01);

	/// <https://www.ietf.org/archive/id/draft-lcurley-moq-transfork-02.html>
	pub const FORK_02: Version = Version(0xff0bad02);

	/// <https://www.ietf.org/archive/id/draft-lcurley-moq-transfork-03.html>
	pub const FORK_03: Version = Version(0xff0bad03);

	/// Unpublished: <https://kixelated.github.io/moq-drafts/draft-lcurley-moq-transfork.html>
	pub const FORK_04: Version = Version(0xff0bad04);

	pub const CURRENT: Version = Version::FORK_04;
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

impl Decode for Version {
	/// Decode the version number.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let v = u64::decode(r)?;
		Ok(Self(v))
	}
}

impl Encode for Version {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.0.encode(w);
	}
}

impl fmt::Debug for Version {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

/// A list of versions in arbitrary order.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Versions(Vec<Version>);

impl Decode for Versions {
	/// Decode the version list.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let count = u64::decode(r)?;
		let mut vs = Vec::new();

		for _ in 0..count {
			let v = Version::decode(r)?;
			vs.push(v);
		}

		Ok(Self(vs))
	}
}

impl Encode for Versions {
	/// Encode the version list.
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.0.len().encode(w);

		for v in &self.0 {
			v.encode(w);
		}
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

impl fmt::Debug for Versions {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_list().entries(self.0.iter()).finish()
	}
}
