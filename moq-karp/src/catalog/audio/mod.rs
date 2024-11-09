mod aac;
mod codec;

pub use aac::*;
pub use codec::*;

use super::{Error, Track};
use serde::{Deserialize, Serialize};
use serde_with::DisplayFromStr;

#[serde_with::serde_as]
#[serde_with::skip_serializing_none]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Audio {
	// Generic information about the track
	pub track: Track,

	#[serde_as(as = "DisplayFromStr")]
	pub codec: AudioCodec,

	pub sample_rate: u16,
	pub channel_count: u16,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub bitrate: Option<u32>,
}
