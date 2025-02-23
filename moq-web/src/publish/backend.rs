use baton::Baton;
use moq_karp::{BroadcastProducer, TrackProducer};
use url::Url;
use wasm_bindgen::JsCast;
use web_sys::MediaStream;

use super::{StatusSend, Video};
use crate::{Connect, ConnectionStatus, Error, Result};

#[derive(Debug, Default, Clone, Baton)]
pub struct Controls {
	pub url: Option<Url>,
	pub volume: f64,
	pub media: Option<MediaStream>,
}

pub struct Backend {
	controls: ControlsRecv,
	status: StatusSend,

	connect: Option<Connect>,
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
						self.connect = Some(Connect::new(url));
						self.status.connection.set(ConnectionStatus::Connecting);
					} else {
						self.connect = None;
						self.status.connection.set(ConnectionStatus::Disconnected);
					}
				},
				Some(session) = async { Some(self.connect.as_mut()?.established().await) } => {
					let path = self.connect.take().unwrap().path;

					let mut broadcast = moq_karp::BroadcastProducer::new(session?, path)?;
					if let Some(video) = self.video.as_mut() {
						self.video_track = Some(broadcast.publish_video(video.info().clone())?);
					}

					self.broadcast = Some(broadcast);
					self.status.connection.set(ConnectionStatus::Connected);
				},
				media = self.controls.media.next() => {
					let media = media.ok_or(Error::Closed)?;

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
					} else {
						self.status.connection.set(ConnectionStatus::Connected);
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
