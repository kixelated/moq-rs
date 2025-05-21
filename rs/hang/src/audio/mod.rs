mod aac;
mod codec;

pub use aac::*;
use bytes::Bytes;
pub use codec::*;

use crate::Track;

use super::Error;
use serde::{Deserialize, Serialize};
use serde_with::{hex::Hex, DisplayFromStr};

#[serde_with::serde_as]
#[serde_with::skip_serializing_none]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
// Mirrors AudioDecoderConfig
// https://w3c.github.io/webcodecs/#audio-decoder-config
pub struct Audio {
	// Generic information about the track
	pub track: Track,

	// The codec, see the registry for details:
	// https://w3c.github.io/webcodecs/codec_registry.html
	#[serde_as(as = "DisplayFromStr")]
	pub codec: AudioCodec,

	// The sample rate of the audio in Hz
	pub sample_rate: u32,

	// The number of channels in the audio
	#[serde(rename = "numberOfChannels")]
	pub channel_count: u32,

	// The bitrate of the audio track in bits per second
	#[serde(default)]
	pub bitrate: Option<u64>,

	// Some codecs include a description so the decoder can be initialized without extra data.
	// If not provided, there may be in-band metadata (marginally higher overhead).
	#[serde(default)]
	#[serde_as(as = "Option<Hex>")]
	pub description: Option<Bytes>,
}
