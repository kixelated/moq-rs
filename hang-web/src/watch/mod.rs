mod audio;
mod message;
mod renderer;
mod video;
mod worklet;

use std::time::Duration;

pub use audio::*;
pub use message::*;
pub use renderer::*;
pub use video::*;
pub use worklet::*;

use crate::{Connect, Result};
use hang::moq_lite;
use moq_lite::Session;

#[derive(Default)]
pub struct Watch {
	connect: Option<Connect>,

	broadcast: Option<hang::BroadcastConsumer>,
	catalog: Option<hang::Catalog>,

	audio: Option<AudioTrack>,
	video: Option<VideoTrack>,

	worklet: Option<web_sys::MessagePort>,
	renderer: Renderer,
}

impl Watch {
	pub fn recv(&mut self, command: WatchCommand) -> Result<()> {
		match command {
			WatchCommand::Connect(url) => {
				self.connect = None;

				if let Some(url) = url {
					self.connect = Some(Connect::new(url));
				}
			}
			WatchCommand::Canvas(canvas) => {
				self.renderer.set_canvas(canvas);
				self.video = self.init_video();
			}
			WatchCommand::Worklet(port) => {
				self.worklet = port;
			}
			WatchCommand::Latency(latency) => self.renderer.set_latency(Duration::from_millis(latency.into())),
			WatchCommand::Paused(paused) => self.renderer.set_paused(paused),
			WatchCommand::Visible(visible) => {
				self.renderer.set_visible(visible);
				self.video = self.init_video();
			}
		};

		Ok(())
	}

	pub async fn run(&mut self) -> Result<()> {
		loop {
			tokio::select! {
				Some(session) = async { Some(self.connect.as_mut()?.established().await) } => {
					let connect = self.connect.take().unwrap();
					self.connected(connect, session?)?;
				}
				Some(res) = async { Some(self.broadcast.as_mut()?.catalog.next().await) } => {
					self.catalog = res?;
					self.video = self.init_video();
				}
				Some(res) = async { Some(self.audio.as_mut()?.frame().await) } => {
					match res? {
						Some(frame) => {},//self.emitter.push(frame),
						None => {self.audio.take();}
					}
				}
				Some(res) = async { Some(self.video.as_mut()?.frame().await) } => {
					match res? {
						Some(frame) => self.renderer.push(frame),
						// Video track has ended.
						None => {self.video.take();}
					}
				}
				// Return Ok() when there's nothing to do.
				else => return Ok(()),
			}
		}
	}

	fn connected(&mut self, connect: Connect, session: Session) -> Result<()> {
		tracing::info!("connected to server");

		let broadcast = connect.path.strip_prefix("/").unwrap();
		let broadcast = session.namespace(broadcast.into());
		self.broadcast = Some(broadcast.into());

		Ok(())
	}

	fn init_video(&mut self) -> Option<VideoTrack> {
		let broadcast = self.broadcast.as_ref()?;
		let catalog = self.catalog.as_ref()?;

		if !self.renderer.should_download() {
			tracing::debug!("canvas not visible, disabling video");
			return None;
		}

		// TODO select the video track based on
		let video = catalog.video.first()?;

		if let Some(existing) = self.video.take() {
			if existing.info.track == video.track {
				return Some(existing);
			}
		}

		let track = broadcast.track(video.track.clone());

		let video = match VideoTrack::new(track, video.clone()) {
			Ok(video) => video,
			Err(err) => {
				tracing::error!(?err, "failed to initialize video track");
				return None;
			}
		};

		if let Some(resolution) = video.info.resolution {
			self.renderer.set_resolution(resolution);
		}

		Some(video)
	}

	fn init_audio(&mut self) -> Option<AudioTrack> {
		let broadcast = self.broadcast.as_ref()?;
		let catalog = self.catalog.as_ref()?;

		let audio = catalog.audio.first()?;

		if let Some(existing) = self.audio.take() {
			if existing.info.track == audio.track {
				return Some(existing);
			}
		}

		let track = broadcast.track(audio.track.clone());

		let audio = match AudioTrack::new(track, audio.clone()) {
			Ok(audio) => audio,
			Err(err) => {
				tracing::error!(?err, "failed to initialize audio track");
				return None;
			}
		};

		Some(audio)
	}
}
