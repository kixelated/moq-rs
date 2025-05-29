use serde::{Deserialize, Serialize};

use crate::model::Location;

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct LocationTrack {
	/// Our initial location.
	///
	/// This is the location that we start with. The viewport is -1 to 1, so 0,0 is the center.
	pub initial: Location,

	/// A track that contains updated locations.
	pub track: Option<moq_lite::Track>,
}
