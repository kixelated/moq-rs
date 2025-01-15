mod av1;
mod codec;
mod dimensions;
mod h264;
mod h265;
mod vp9;

pub use av1::*;
pub use codec::*;
pub use dimensions::*;
pub use h264::*;
pub use h265::*;
pub use vp9::*;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_with::{hex::Hex, DisplayFromStr};

use crate::Track;

#[serde_with::serde_as]
#[serde_with::skip_serializing_none]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Video {
	// Generic information about the track
	pub track: Track,

	// The codec mimetype encoded as a string
	// ex. avc1.42c01e
	#[serde_as(as = "DisplayFromStr")]
	pub codec: VideoCodec,

	// Some codecs unfortunately aren't self-describing
	// One of the best examples is H264, which needs the sps/pps out of band to function.
	#[serde(default)]
	#[serde_as(as = "Option<Hex>")]
	pub description: Option<Bytes>,

	// The encoded width/height of the media
	pub resolution: Dimensions,

	#[serde(default)]
	pub bitrate: Option<u64>,
}
