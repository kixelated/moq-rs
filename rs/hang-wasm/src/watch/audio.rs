use web_async::FuturesExt;
use web_message::Message;

use rubato::{FastFixedIn, Resampler};

use crate::{Result, WorkletCommand};

#[derive(Default)]
pub struct Audio {
	worklet: Option<web_sys::MessagePort>,
	track: Option<AudioTrack>,

	sample_rate: u32,
	resampler: Option<FastFixedIn<f32>>,
	resampler_in: Vec<Vec<f32>>,
	resampler_out: Vec<Vec<f32>>,

	muted: bool,

	broadcast: Option<hang::BroadcastConsumer>,
	catalog: Option<hang::Catalog>,
}

impl Audio {
	// Returns a useless Option so we can short-circuit if audio is disabled.
	fn reinit(&mut self) -> Option<()> {
		let existing = self.track.take();

		if self.muted || self.worklet.is_none() {
			return Some(());
		}

		let broadcast = self.broadcast.as_ref()?;
		let catalog = self.catalog.as_ref()?;
		let audio = catalog.audio.first()?;

		if let Some(existing) = existing {
			if existing.info.track == audio.track {
				// Reuse the existing subscription (avoid skipping).
				self.track = Some(existing);
				return Some(());
			}
		}

		let track = broadcast.track(&audio.track);

		// TODO handle the error instead of ignoring it.
		let track = AudioTrack::new(track, audio.clone()).ok()?;

		tracing::info!(info = ?track.info, "loaded audio track");
		self.track = Some(track);

		Some(())
	}

	pub fn set_catalog(&mut self, broadcast: Option<hang::BroadcastConsumer>, catalog: Option<hang::Catalog>) {
		self.broadcast = broadcast;
		self.catalog = catalog;
		self.reinit();
	}

	pub fn set_worklet(&mut self, worklet: Option<web_sys::MessagePort>, sample_rate: u32) {
		self.worklet = worklet;
		self.sample_rate = sample_rate;

		if let Some(worklet) = self.worklet.as_ref() {
			// TODO addEventLister
			worklet.start();
		}

		self.reinit();
	}

	pub fn set_muted(&mut self, muted: bool) {
		self.muted = muted;
		self.reinit();
	}

	pub async fn run(&mut self) {
		if let Err(err) = self.run_inner().await {
			tracing::error!(?err, "error running audio; disabling");
		}

		// Prevent infinite loops by disabling the track.
		self.track.take();

		// Block indefinitely so we don't break out of the parent select! loop.
		// This is a hack as we're abusing select! to run tasks in parallel but ignore the results.
		std::future::pending::<()>().await;
	}

	async fn run_inner(&mut self) -> Result<()> {
		while let Some(track) = self.track.as_mut() {
			let frame = match track.frame().await? {
				Some(frame) => frame,
				None => return Ok(()),
			};

			// Resample the audio frame if needed.
			if frame.sample_rate() != self.sample_rate {
				self.resample(frame)?;
			} else {
				self.proxy(frame)?;
			}
		}

		Ok(())
	}

	fn resample(&mut self, frame: web_codecs::AudioData) -> Result<()> {
		let worklet = match self.worklet.as_ref() {
			Some(worklet) => worklet,
			None => return Ok(()),
		};

		tracing::trace!(
			channels = ?frame.number_of_channels(),
			sample_rate = ?frame.sample_rate(),
			new_sample_rate = self.sample_rate,
			timestamp = ?frame.timestamp(),
			"resampling audio frame"
		);

		if self.resampler.is_none() {
			let ratio = self.sample_rate as f64 / frame.sample_rate() as f64;
			let resampler = FastFixedIn::new(
				ratio,
				1.0,
				rubato::PolynomialDegree::Cubic, // TODO benchmark?
				frame.number_of_frames() as _,
				frame.number_of_channels() as _,
			)?;

			self.resampler_in = resampler.input_buffer_allocate(false);
			self.resampler_out = resampler.output_buffer_allocate(true);
			self.resampler = Some(resampler);
		}

		// Copy the audio data to the resampler input buffer.
		for (i, dst) in self.resampler_in.iter_mut().enumerate() {
			frame.append_to(dst, i, Default::default())?;
		}

		loop {
			// Process the audio data into the output buffer.
			let resampler = self.resampler.as_mut().unwrap();

			if self.resampler_in[0].len() < resampler.input_frames_next() {
				// Not enough space in the input buffer, wait for more data.
				break;
			}

			let (input_samples, output_samples) =
				resampler.process_into_buffer(&self.resampler_in, &mut self.resampler_out, None)?;

			for channel in self.resampler_in.iter_mut() {
				// Reclaim space in the input buffer.
				channel.drain(..input_samples);
			}

			if output_samples == 0 {
				continue;
			}

			let channels = self
				.resampler_out
				.iter()
				.map(|channel| {
					let array = js_sys::Float32Array::new_with_length(output_samples as _);
					array.copy_from(&channel[..output_samples]);
					array
				})
				.collect();

			let command = WorkletCommand::Frame {
				channels,
				timestamp: frame.timestamp().as_micros() as u64,
			};

			let mut transfer = js_sys::Array::new();
			let msg = command.into_message(&mut transfer);
			worklet.post_message_with_transferable(&msg, &transfer)?;
		}

		Ok(())
	}

	// Send the audio frame to the worklet directly if the sample rate is the same.
	fn proxy(&mut self, frame: web_codecs::AudioData) -> Result<()> {
		let worklet = match self.worklet.as_ref() {
			Some(worklet) => worklet,
			None => return Ok(()),
		};

		tracing::trace!(
			channels = ?frame.number_of_channels(),
			sample_rate = ?frame.sample_rate(),
			timestamp = ?frame.timestamp(),
			"transferring audio frame"
		);

		let channels = (0..frame.number_of_channels() as usize)
			.map(|i| {
				let mut array = js_sys::Float32Array::new_with_length(frame.number_of_frames() as _);
				frame.copy_to(&mut array, i, Default::default())?;
				Ok(array)
			})
			.collect::<Result<Vec<_>>>()?;

		let command = WorkletCommand::Frame {
			channels,
			timestamp: frame.timestamp().as_micros() as u64,
		};
		let mut transfer = js_sys::Array::new();
		let msg = command.into_message(&mut transfer);
		worklet.post_message_with_transferable(&msg, &transfer)?;

		Ok(())
	}
}

pub struct AudioTrack {
	pub track: hang::TrackConsumer,
	pub info: hang::AudioTrack,

	decoder: web_codecs::AudioDecoder,
	decoded: web_codecs::AudioDecoded,
}

impl AudioTrack {
	pub fn new(track: hang::TrackConsumer, info: hang::AudioTrack) -> Result<Self> {
		let config = &info.config;

		// Construct the video decoder
		let (decoder, decoded) = web_codecs::AudioDecoderConfig {
			codec: config.codec.to_string(),
			description: config.description.clone(),
			channel_count: config.channel_count,
			sample_rate: config.sample_rate,
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
