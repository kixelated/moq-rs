mod av1;
mod codec;
mod h264;
mod h265;
mod vp9;

pub use av1::*;
pub use codec::*;
pub use h264::*;
pub use h265::*;
pub use vp9::*;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_with::{hex::Hex, DisplayFromStr};

#[serde_with::serde_as]
#[serde_with::skip_serializing_none]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
/// Information about a video track.
pub struct VideoTrack {
	/// MoQ specific track information
	pub track: moq_lite::Track,

	/// The configuration of the video track
	pub config: VideoConfig,
}

#[serde_with::serde_as]
#[serde_with::skip_serializing_none]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
/// VideoDecoderConfig from WebCodecs
/// https://w3c.github.io/webcodecs/#video-decoder-config
pub struct VideoConfig {
	/// The codec, see the registry for details:
	/// https://w3c.github.io/webcodecs/codec_registry.html
	#[serde_as(as = "DisplayFromStr")]
	pub codec: VideoCodec,

	/// Information used to initialize the decoder on a per-codec basis.
	///
	/// One of the best examples is H264, which needs the sps/pps to function.
	/// If not provided, this information is (automatically) inserted before each key-frame (marginally higher overhead).
	#[serde(default)]
	#[serde_as(as = "Option<Hex>")]
	pub description: Option<Bytes>,

	/// The encoded width/height of the media.
	///
	/// This is optional because it can be changed in-band for some codecs.
	/// It's primarily a hint to allocate the correct amount of memory up-front.
	pub coded_width: Option<u32>,
	pub coded_height: Option<u32>,

	/// The display aspect ratio of the media.
	///
	/// This allows you to stretch/shrink pixels of the video.
	/// If not provided, the display aspect ratio is 1:1
	pub display_ratio_width: Option<u32>,
	pub display_ratio_height: Option<u32>,

	// TODO color space
	/// The maximum bitrate of the video track, if known.
	#[serde(default)]
	pub bitrate: Option<u64>,

	/// The frame rate of the video track, if known.
	#[serde(default)]
	pub framerate: Option<f64>,

	/// If true, the decoder will optimize for latency.
	///
	/// Default: true
	#[serde(default)]
	pub optimize_for_latency: Option<bool>,

	// The rotation of the video in degrees
	// Default: 0
	#[serde(default)]
	pub rotation: Option<f64>,

	// If true, the decoder will flip the video horizontally
	// Default: false
	#[serde(default)]
	pub flip: Option<bool>,
}
