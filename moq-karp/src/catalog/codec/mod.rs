use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

mod av1;
mod error;
mod h264;
mod h265;
mod vp8;
mod vp9;

pub use av1::*;
pub use error::*;
pub use h264::*;
pub use h265::*;
pub use vp8::*;
pub use vp9::*;

macro_rules! video_codec {
	{$($name:ident,)*} => {
		#[serde_with::serde_as]
		#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
		pub enum VideoCodec {
			$($name($name),)*

			#[serde(untagged)]
			Unknown(String),
		}

		$(
			impl From<$name> for VideoCodec {
				fn from(codec: $name) -> Self {
					Self::$name(codec)
				}
			}
		)*

		impl fmt::Display for VideoCodec {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				match self {
					$(
						Self::$name(codec) => codec.fmt(f),
					)*
					Self::Unknown(codec) => codec.fmt(f),
				}
			}
		}

		impl FromStr for VideoCodec {
			type Err = CodecError;

			fn from_str(s: &str) -> Result<Self, Self::Err> {
				$(
					if s.starts_with($name::PREFIX) {
						return $name::from_str(s).map(Into::into)
					}
				)*

				Ok(Self::Unknown(s.to_string()))
			}
		}
	};
}

video_codec! {
	H264,
	H265,
	VP8,
	VP9,
	AV1,
}
