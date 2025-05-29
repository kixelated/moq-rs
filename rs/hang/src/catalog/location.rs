use serde::{Deserialize, Serialize};

use crate::model::Location;

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct LocationTrack {
	/// Our initial location.
	///
	/// This is the location that we start with. The range is -1 to 1 with (0,0) being the center.
	pub initial: Location,

	/// An optional track that contains live location updates.
	pub track: Option<moq_lite::Track>,

	/// If present, viewers can drag this location via this handle and a feedback track.
	pub handle: Option<u32>,
}
