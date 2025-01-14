use wasm_bindgen::prelude::*;
use web_sys::MediaStream;

use super::{Audio, Video};

pub struct Media {
	_audio: Vec<Audio>,
	_video: Vec<Video>,
}

impl Media {
	pub fn new(broadcast: moq_karp::BroadcastProducer, media: MediaStream) -> Self {
		let _audio = Vec::new();
		let mut _video = Vec::new();

		// TODO listen for updates
		for track in media.get_tracks() {
			let track: web_sys::MediaStreamTrack = track.unchecked_into();

			match track.kind().as_str() {
				"audio" => {
					tracing::warn!("skipping audio track");
				}
				"video" => {
					_video.push(Video::new(broadcast.clone(), track));
				}
				_ => {
					tracing::warn!("unknown track kind: {:?}", track.kind());
				}
			}
		}

		Self { _audio, _video }
	}
}
