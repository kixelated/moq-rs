use std::{fmt, str::FromStr};

/// A subset of jsonwebtoken algorithms.
///
/// We could support all of them, but there's currently no point using public key crypto.
/// The relay can fetch any resource it wants; it doesn't need to forge tokens.
///
/// TODO support public key crypto at some point.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum Algorithm {
	HS256,
	HS384,
	HS512,
}

impl Into<jsonwebtoken::Algorithm> for Algorithm {
	fn into(self) -> jsonwebtoken::Algorithm {
		match self {
			Algorithm::HS256 => jsonwebtoken::Algorithm::HS256,
			Algorithm::HS384 => jsonwebtoken::Algorithm::HS384,
			Algorithm::HS512 => jsonwebtoken::Algorithm::HS512,
		}
	}
}

impl FromStr for Algorithm {
	type Err = anyhow::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"HS256" => Ok(Algorithm::HS256),
			"HS384" => Ok(Algorithm::HS384),
			"HS512" => Ok(Algorithm::HS512),
			_ => anyhow::bail!("invalid algorithm: {}", s),
		}
	}
}

impl fmt::Display for Algorithm {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Algorithm::HS256 => write!(f, "HS256"),
			Algorithm::HS384 => write!(f, "HS384"),
			Algorithm::HS512 => write!(f, "HS512"),
		}
	}
}
