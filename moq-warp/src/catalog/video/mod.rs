use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_with::{hex::Hex, DisplayFromStr};

use std::fmt;
use std::str::FromStr;

use super::{CodecError, Dimensions};

mod av1;
mod h264;
mod h265;
mod vp8;
mod vp9;

pub use av1::*;
pub use h264::*;
pub use h265::*;
pub use vp8::*;
pub use vp9::*;

#[serde_with::serde_as]
#[serde_with::skip_serializing_none]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Video {
	pub track: moq_transfork::Track,

	// The codec mimetype encoded as a string
	// ex. avc1.42c01e
	#[serde_as(as = "DisplayFromStr")]
	pub codec: VideoCodec,

	// Some codecs unfortunately aren't self-describing
	// One of the best examples is H264, which needs the sps/pps out of band to function.
	#[serde(default, skip_serializing_if = "Bytes::is_empty")]
	#[serde_as(as = "Hex")]
	pub description: Bytes,

	// The encoded width/height of the media
	pub resolution: Dimensions,

	// The number of units in a second
	pub timescale: u32,

	#[serde(default)]
	pub bitrate: Option<u32>,

	#[serde(default)]
	pub display: Option<Dimensions>,

	// Additional enhancement layers
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub layers: Vec<VideoLayer>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VideoLayer {
	pub track: moq_transfork::Track,
	pub dimensions: Dimensions,
}

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
