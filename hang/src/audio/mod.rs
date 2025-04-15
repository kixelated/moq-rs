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
pub struct Audio {
	// Generic information about the track
	pub track: Track,

	#[serde_as(as = "DisplayFromStr")]
	pub codec: AudioCodec,

	pub sample_rate: u32,
	pub channel_count: u32,

	// The bitrate of the audio track
	#[serde(default)]
	pub bitrate: Option<u64>,

	// Some codecs unfortunately aren't self-describing and include extra metadata.
	// For example, AAC can include an ADTS header out of band.
	#[serde(default)]
	#[serde_as(as = "Option<Hex>")]
	pub description: Option<Bytes>,
}
