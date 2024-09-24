use bytes::{Bytes, BytesMut};
use mp4_atom::Encode;
use serde::{Deserialize, Serialize};
use serde_with::hex::Hex;

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
	pub codec: VideoCodec,
	pub dimensions: Dimensions,

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

		impl std::fmt::Display for VideoCodec {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				match self {
					$(
						Self::$name(codec) => codec.fmt(f),
					)*
					Self::Unknown(codec) => codec.fmt(f),
				}
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

macro_rules! video_description {
	{$($name:ident,)*} => {
		impl VideoCodec {
			pub fn description(&self) -> Option<Bytes> {
				match self {
					$( Self::$name(codec) => Some(codec.description()), )*
					_ => None,
				}
			}
		}
	}
}

// Codecs that have a description() method
video_description! {
	H264,
}
