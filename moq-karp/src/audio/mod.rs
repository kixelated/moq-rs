mod aac;
mod codec;

pub use aac::*;
pub use codec::*;

use crate::Track;

use super::Error;
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

	pub sample_rate: u32,
	pub channel_count: u32,

	pub bitrate: Option<u64>,
}
