mod audio;
mod command;
mod video;

pub use audio::*;
pub use command::*;
pub use video::*;

use hang::{moq_lite::Session, BroadcastProducer};

use crate::{Connect, Result};

#[derive(Default)]
pub struct Publish {
	connect: Option<Connect>,
	broadcast: Option<BroadcastProducer>,

	audio: Option<PublishAudio>,
	audio_id: usize,

	video: Option<PublishVideo>,
	video_id: usize,
}

impl Publish {
	pub async fn recv(&mut self, command: PublishCommand) -> Result<()> {
		match command {
			PublishCommand::Connect(url) => {
				self.connect = None;

				if let Some(url) = url {
					self.connect = Some(Connect::new(url)?);
				}
			}
			PublishCommand::VideoInit { width, height } => {
				self.video = Some(PublishVideo::init(self.video_id, width, height).await?);
				self.video_id += 1;

				if let Some(broadcast) = self.broadcast.as_mut() {
					self.video.as_mut().unwrap().publish_to(broadcast);
				}
			}
			PublishCommand::VideoFrame(frame) => {
				// Don't encode anything until connecting to a room, for privacy reasons.
				match self.video.as_mut() {
					Some(video) => video.encode(frame).await?,
					None => frame.close(),
				};
			}
			PublishCommand::VideoClose => {
				self.video = None;
			}
			PublishCommand::AudioInit {
				sample_rate,
				channel_count,
			} => {
				self.audio = Some(PublishAudio::init(self.audio_id, channel_count, sample_rate).await?);
				self.audio_id += 1;

				if let Some(broadcast) = self.broadcast.as_mut() {
					self.audio.as_mut().unwrap().publish_to(broadcast);
				}
			}
			PublishCommand::AudioFrame(frame) => {
				match self.audio.as_mut() {
					Some(audio) => audio.encode(frame).await?,
					None => frame.close(),
				};
			}
			PublishCommand::AudioClose => {
				self.audio = None;
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
				Some(Err(err)) = async { Some(self.audio.as_mut()?.run().await) } => return Err(err),
				Some(Err(err)) = async { Some(self.video.as_mut()?.run().await) } => return Err(err),
				else => return Ok(()),
			};
		}
	}

	fn connected(&mut self, connect: Connect, session: Session) -> Result<()> {
		tracing::info!("connected to server");

		let path = connect.path.strip_prefix("/").unwrap().to_string();
		let mut room = hang::Room::new(session, path.to_string());
		let mut broadcast = room.join(path);

		if let Some(video) = self.video.as_mut() {
			video.publish_to(&mut broadcast);
		}

		if let Some(audio) = self.audio.as_mut() {
			audio.publish_to(&mut broadcast);
		}

		Ok(())
	}
}
