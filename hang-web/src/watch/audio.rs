use web_async::FuturesExt;
use web_message::Message;

use rubato::{FftFixedIn, Resampler};

use crate::{Error, Result, WorkletCommand};

#[derive(Default)]
pub struct Audio {
	worklet: Option<web_sys::MessagePort>,
	track: Option<AudioTrack>,

	sample_rate: u32,
	resampler: Option<FftFixedIn<f32>>,
	resampler_in: Vec<Vec<f32>>,
	resampler_out: Vec<Vec<f32>>,
}

impl Audio {
	pub fn set_worklet(&mut self, worklet: Option<web_sys::MessagePort>) {
		self.worklet = worklet;

		if let Some(worklet) = self.worklet.as_ref() {
			// TODO addEventLister
			worklet.start();
		}

		// Call init now
	}

	// Resample audio to the given sample rate.
	pub fn set_sample_rate(&mut self, sample_rate: u32) {
		self.sample_rate = sample_rate;
	}

	// Returns an error indicating why audio was disabled.
	pub fn init(&mut self, broadcast: Option<&hang::BroadcastConsumer>, catalog: Option<&hang::Catalog>) -> Result<()> {
		let existing = self.track.take();

		let broadcast = broadcast.ok_or(Error::NoBroadcast)?;
		let catalog = catalog.ok_or(Error::NoCatalog)?;
		let audio = catalog.audio.first().ok_or(Error::NoTrack)?;

		if let Some(existing) = existing {
			if existing.info.track == audio.track {
				// Reuse the existing subscription (avoid skipping).
				self.track = Some(existing);
				return Ok(());
			}
		}

		let track = broadcast.track(audio.track.clone());

		let track = AudioTrack::new(track, audio.clone())?;

		tracing::info!(info = ?track.info, "loaded audio track");
		self.track = Some(track);

		Ok(())
	}

	pub async fn run(&mut self) -> Result<()> {
		let track = match self.track.as_mut() {
			Some(track) => track,
			None => return Ok(()),
		};

		let worklet = match self.worklet.as_ref() {
			Some(worklet) => worklet,
			None => return Ok(()),
		};

		loop {
			let mut frame = match track.frame().await? {
				Some(frame) => frame,
				None => {
					// Close the track.
					self.track.take();
					return Ok(());
				}
			};

			// Resample the audio frame if needed.
			if frame.sample_rate() != self.sample_rate {
				tracing::trace!(
					channels = ?frame.number_of_channels(),
					sample_rate = ?frame.sample_rate(),
					new_sample_rate = self.sample_rate,
					timestamp = ?frame.timestamp(),
					"resampling audio frame"
				);

				if self.resampler.is_none() {
					let resampler = FftFixedIn::new(
						frame.sample_rate() as _,
						self.sample_rate as _,
						frame.number_of_frames() as _,
						1,
						frame.number_of_channels() as _,
					)?;

					self.resampler_in = resampler.input_buffer_allocate(true);
					self.resampler_out = resampler.output_buffer_allocate(true);
					self.resampler = Some(resampler);
				}

				// Copy the audio data to the resampler input buffer.
				for (i, dst) in self.resampler_in.iter_mut().enumerate() {
					frame.copy_to(dst.as_mut_slice(), i, Default::default())?;
				}

				// Process the audio data into the output buffer.
				let resampler = self.resampler.as_mut().unwrap();
				let (input, output) =
					resampler.process_into_buffer(&self.resampler_in, &mut self.resampler_out, None)?;

				// Sanity check to make sure we processed every sample.
				assert_eq!(input, frame.number_of_frames() as usize);

				if output == 0 {
					continue;
				}

				// Create a new audio frame with the resampled data.
				let output = self.resampler_out.iter().map(|v| &v[..output]);
				frame = web_codecs::AudioData::new(output, self.sample_rate, frame.timestamp())?;
			}

			tracing::trace!(
				channels = ?frame.number_of_channels(),
				sample_rate = ?frame.sample_rate(),
				timestamp = ?frame.timestamp(),
				"transferring audio frame"
			);

			let command = WorkletCommand::Frame(frame.into());
			let mut transfer = js_sys::Array::new();
			let msg = command.into_message(&mut transfer);
			worklet.post_message_with_transferable(&msg, &transfer)?;
		}
	}
}

pub struct AudioTrack {
	pub track: hang::TrackConsumer,
	pub info: hang::Audio,

	decoder: web_codecs::AudioDecoder,
	decoded: web_codecs::AudioDecoded,
}

impl AudioTrack {
	pub fn new(track: hang::TrackConsumer, info: hang::Audio) -> Result<Self> {
		// Construct the video decoder
		let (decoder, decoded) = web_codecs::AudioDecoderConfig {
			codec: info.codec.to_string(),
			description: info.description.clone(),
			channel_count: info.channel_count,
			sample_rate: info.sample_rate,
			..Default::default()
		}
		.build()?;

		Ok(Self {
			track,
			info,
			decoder,
			decoded,
		})
	}

	pub async fn frame(&mut self) -> Result<Option<web_codecs::AudioData>> {
		loop {
			tokio::select! {
				Some(frame) = self.track.read().transpose() => {
					let frame = frame?;

					let frame = web_codecs::EncodedFrame {
						payload: frame.payload,
						timestamp: frame.timestamp,
						keyframe: frame.keyframe,
					};

					self.decoder.decode(frame)?;
				},
				Some(frame) = self.decoded.next().transpose() => return Ok(Some(frame?)),
				else => return Ok(None),
			}
		}
	}
}
