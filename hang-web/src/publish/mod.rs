mod audio;
mod command;
mod video;

pub use audio::*;
pub use command::*;
pub use video::*;

use hang::{BroadcastProducer, Room};

use crate::Result;

pub struct Publish {
	room: Option<Room>,
	name: Option<String>,

	broadcast: Option<BroadcastProducer>,

	audio: Option<PublishAudio>,
	audio_id: usize,

	video: Option<PublishVideo>,
	video_id: usize,
}

impl Default for Publish {
	fn default() -> Self {
		Self::new()
	}
}

impl Publish {
	pub fn new() -> Self {
		Self {
			room: None,
			broadcast: None,
			name: None,
			audio: None,
			audio_id: 0,
			video: None,
			video_id: 0,
		}
	}

	pub async fn recv_command(&mut self, command: PublishCommand) -> Result<()> {
		match command {
			PublishCommand::Name(name) => {
				self.name = name;
				self.create_broadcast()?;
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

	pub fn set_room(&mut self, room: Option<Room>) -> Result<()> {
		self.room = room;
		self.create_broadcast()
	}

	fn create_broadcast(&mut self) -> Result<()> {
		if let (Some(room), Some(name)) = (self.room.as_mut(), self.name.as_ref()) {
			let mut broadcast = room.join(name.clone())?;

			if let Some(video) = self.video.as_mut() {
				video.publish_to(&mut broadcast);
			}

			if let Some(audio) = self.audio.as_mut() {
				audio.publish_to(&mut broadcast);
			}

			self.broadcast = Some(broadcast);
		} else {
			self.broadcast = None;
		}

		Ok(())
	}

	pub async fn run(&mut self) -> Result<()> {
		tokio::select! {
			Some(Err(err)) = async { Some(self.audio.as_mut()?.run().await) } => Err(err),
			Some(Err(err)) = async { Some(self.video.as_mut()?.run().await) } => Err(err),
			else => Ok(()),
		}
	}
}
