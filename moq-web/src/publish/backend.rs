use moq_karp::{moq_transfork::Path, BroadcastProducer, TrackProducer};
use wasm_bindgen::JsCast;

use super::{ControlsRecv, PublishState, StatusSend, Video};
use crate::{Connect, Error, Result};

pub struct Backend {
	controls: ControlsRecv,
	status: StatusSend,

	connect: Option<Connect>,
	path: Path,
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
			path: Path::default(),
			broadcast: None,
			video: None,
			video_track: None,
		}
	}

	pub fn start(mut self) {
		wasm_bindgen_futures::spawn_local(async move {
			if let Err(err) = self.run().await {
				tracing::error!(?err, "backend error");
				self.status.error.set(Some(err));
			}

			self.status.state.set(PublishState::Closed);
		});
	}

	pub async fn run(&mut self) -> Result<()> {
		loop {
			let connect = self.connect.as_mut();
			let mut video = self.video.as_mut();

			tokio::select! {
				Some(Some(url)) = self.controls.url.next() => {
					// Connect using the base of the URL.
					let mut addr = url.clone();
					addr.set_fragment(None);
					addr.set_query(None);
					addr.set_path("");

					self.path = url.path_segments().ok_or(Error::InvalidUrl)?.collect();
					self.connect = Some(Connect::new(addr));

					self.status.state.set(PublishState::Connecting);
				},
				Some(session) = async move { Some(connect?.established().await) } => {
					let mut broadcast = moq_karp::BroadcastProducer::new(session?, self.path.clone())?;
					if let Some(video) = self.video.as_mut() {
						self.video_track = Some(broadcast.publish_video(video.info().clone())?);
					}

					self.broadcast = Some(broadcast);
					self.connect = None;

					self.status.state.set(PublishState::Connected);
				},
				Some(true) = self.controls.close.next() => return Ok(()),
				Some(media) = self.controls.media.next() => {
					// Close the existing video stream.
					self.video.take();
					self.video_track.take();

					if let Some(media) = media {
						if let Some(track) = media.get_video_tracks().iter().next() {
							let track: web_sys::MediaStreamTrack = track.unchecked_into();

							// TODO: Perform this async if the delay is noticeable.
							let video = Video::new(track).await?;

							if let Some(broadcast) = self.broadcast.as_mut() {
								self.video_track = Some(broadcast.publish_video(video.info().clone())?);
							}

							self.video = Some(video);
						}
					}
				},
				Some(frame) = async move { Some(video.as_mut()?.frame().await) } => {
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
