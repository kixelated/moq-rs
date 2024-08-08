use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Track {
	pub namespace: String,

	pub name: String,

	#[serde(rename = "initTrack", skip_serializing_if = "Option::is_none")]
	pub init_track: Option<String>,

	#[serde(rename = "initData", skip_serializing_if = "Option::is_none")]
	#[serde_as(as = "Option<serde_with::hex::Hex>")]
	pub init_data: Option<Vec<u8>>,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub packaging: Option<TrackPackaging>,

	#[serde(rename = "renderGroup", skip_serializing_if = "Option::is_none")]
	pub render_group: Option<u16>,

	#[serde(rename = "altGroup", skip_serializing_if = "Option::is_none")]
	pub alt_group: Option<u16>,

	#[serde(rename = "selectionParams")]
	pub selection_params: SelectionParam,

	#[serde(rename = "temporalId", skip_serializing_if = "Option::is_none")]
	pub temporal_id: Option<u32>,

	#[serde(rename = "spatialId", skip_serializing_if = "Option::is_none")]
	pub spatial_id: Option<u32>,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub depends: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub enum TrackPackaging {
	#[serde(rename = "cmaf")]
	#[default]
	Cmaf,

	#[serde(rename = "loc")]
	Loc,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct SelectionParam {
	pub codec: Option<String>,

	#[serde(rename = "mimeType")]
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mime_type: Option<String>,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub framerate: Option<u64>,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub bitrate: Option<u32>,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub width: Option<u32>,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub height: Option<u32>,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub samplerate: Option<u32>,

	#[serde(rename = "channelConfig", skip_serializing_if = "Option::is_none")]
	pub channel_config: Option<String>,

	#[serde(rename = "displayWidth", skip_serializing_if = "Option::is_none")]
	pub display_width: Option<u16>,

	#[serde(rename = "displayHeight", skip_serializing_if = "Option::is_none")]
	pub display_height: Option<u16>,

	#[serde(rename = "lang", skip_serializing_if = "Option::is_none")]
	pub language: Option<String>,
}
