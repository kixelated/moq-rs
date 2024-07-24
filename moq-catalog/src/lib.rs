//! This module contains the structs and functions for the MoQ catalog format
/// The catalog format is a JSON file that describes the tracks available in a broadcast.
///
/// The current version of the catalog format is draft-01.
/// https://www.ietf.org/archive/id/draft-ietf-moq-catalogformat-01.html
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Root {
	pub version: u16,

	#[serde(rename = "streamingFormat")]
	pub streaming_format: u16,

	#[serde(rename = "streamingFormatVersion")]
	pub streaming_format_version: String,

	#[serde(rename = "supportsDeltaUpdates")]
	pub streaming_delta_updates: bool,

	#[serde(rename = "commonTrackFields")]
	pub common_track_fields: CommonTrackFields,

	pub tracks: Vec<Track>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Track {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub namespace: Option<String>,

	pub name: String,

	#[serde(rename = "initTrack", skip_serializing_if = "Option::is_none")]
	pub init_track: Option<String>,

	#[serde(rename = "initData", skip_serializing_if = "Option::is_none")]
	pub init_data: Option<String>,

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

impl Track {
	#[allow(dead_code)] // TODO use
	fn with_common(&mut self, common: &CommonTrackFields) {
		if self.namespace.is_none() {
			self.namespace.clone_from(&common.namespace);
		}
		if self.packaging.is_none() {
			self.packaging.clone_from(&common.packaging);
		}
		if self.render_group.is_none() {
			self.render_group = common.render_group;
		}
		if self.alt_group.is_none() {
			self.alt_group = common.alt_group;
		}
	}
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub enum TrackPackaging {
	#[serde(rename = "cmaf")]
	#[default]
	Cmaf,

	#[serde(rename = "loc")]
	Loc,
}

#[derive(Serialize, Deserialize, Debug, Default)]
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

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct CommonTrackFields {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub namespace: Option<String>,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub packaging: Option<TrackPackaging>,

	#[serde(rename = "renderGroup", skip_serializing_if = "Option::is_none")]
	pub render_group: Option<u16>,

	#[serde(rename = "altGroup", skip_serializing_if = "Option::is_none")]
	pub alt_group: Option<u16>,
}

impl CommonTrackFields {
	/// Serialize function to conditionally include fields based on their commonality amoung tracks
	pub fn from_tracks(tracks: &mut [Track]) -> Self {
		if tracks.is_empty() {
			return Default::default();
		}

		// Use the first track as the basis
		let mut common = Self {
			namespace: tracks[0].namespace.clone(),
			packaging: tracks[0].packaging.clone(),
			render_group: tracks[0].render_group,
			alt_group: tracks[0].alt_group,
		};

		// Loop over the other tracks to check if they have the same values
		for track in &mut tracks[1..] {
			if track.namespace != common.namespace {
				common.namespace = None;
			}
			if track.packaging != common.packaging {
				common.packaging = None;
			}
			if track.render_group != common.render_group {
				common.render_group = None
			}
			if track.alt_group != common.alt_group {
				common.alt_group = None;
			}
		}

		// Loop again to remove the common fields from the tracks
		for track in tracks {
			if common.namespace.is_some() {
				track.namespace = None;
			}
			if track.packaging.is_some() {
				track.packaging = None;
			}
			if track.render_group.is_some() {
				track.render_group = None;
			}
			if track.alt_group.is_some() {
				track.alt_group = None;
			}
		}

		common
	}
}
