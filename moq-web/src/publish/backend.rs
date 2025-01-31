use baton::Baton;
use moq_karp::{BroadcastProducer, TrackProducer};
use url::Url;
use wasm_bindgen::JsCast;
use web_sys::MediaStream;

use super::{PublishState, Video};
use crate::{Connect, Error, Result};

#[derive(Debug, Default, Clone, Baton)]
pub struct Controls {
	pub url: Option<Url>,
	pub volume: f64,
	pub media: Option<MediaStream>,
}

#[derive(Debug, Default, Clone, Baton)]
pub struct Status {
	pub state: PublishState,
	pub error: Option<Error>,
}

pub struct Backend {
	controls: ControlsRecv,
	status: StatusSend,

	connect: Option<Connect>,
	path: Option<String>,
	broadcast: Option<BroadcastProducer>,

	video: Option<Video>,
	video_track: Option<TrackProducer>,
}

impl Backend {
	pub fn new(controls: ControlsRecv, status: StatusSend) -> Self {
		Self {
			controls,
			status,
			connect: None,
			path: None,
			broadcast: None,
			video: None,
			video_track: None,
		}
	}

	pub fn start(mut self) {
		wasm_bindgen_futures::spawn_local(async move {
			if let Err(err) = self.run().await {
				self.status.error.set(Some(err));
			}

			self.status.state.set(PublishState::Error);
		});
	}

	pub async fn run(&mut self) -> Result<()> {
		loop {
			tokio::select! {
				url = self.controls.url.next() => {
					let url = url.ok_or(Error::Closed)?;

					// Close the current broadcast.
					self.broadcast = None;
					self.video = None;

					if let Some(url) = url {
						// Connect using the base of the URL.
						let mut addr = url.clone();
						addr.set_fragment(None);
						addr.set_query(None);
						addr.set_path("");

						self.path = Some(url.path().to_string());
						self.connect = Some(Connect::new(addr));

						self.status.state.set(PublishState::Connecting);
					} else {
						self.path = None;
						self.connect = None;

						self.status.state.set(PublishState::Idle);
					}
				},
				Some(session) = async { Some(self.connect.as_mut()?.established().await) } => {
					let path = self.path.as_ref().unwrap();
					let mut broadcast = moq_karp::BroadcastProducer::new(session?, path.to_string())?;
					if let Some(video) = self.video.as_mut() {
						self.video_track = Some(broadcast.publish_video(video.info().clone())?);
					}

					self.broadcast = Some(broadcast);
					self.connect = None;

					self.status.state.set(PublishState::Connected);
				},
				media = self.controls.media.next() => {
					let media = media.ok_or(Error::Closed)?;

					// Close the existing video stream.
					self.video.take();
					self.video_track.take();

					if let Some(media) = media {
						self.status.state.set(PublishState::Live);
						if let Some(track) = media.get_video_tracks().iter().next() {
							let track: web_sys::MediaStreamTrack = track.unchecked_into();

							// TODO: Perform this async if the delay is noticeable.
							let video = Video::new(track).await?;

							if let Some(broadcast) = self.broadcast.as_mut() {
								self.video_track = Some(broadcast.publish_video(video.info().clone())?);
							}

							self.video = Some(video);
						}
					} else {
						self.status.state.set(PublishState::Connected);
					}
				},
				Some(frame) = async { Some(self.video.as_mut()?.frame().await) } => {
					match frame? {
						None => { self.video = None; }
						Some(frame) => {
							if let Some(track) = self.video_track.as_mut() {
								track.write(frame);
							}
						},
					}
				},
			}
		}
	}
}
