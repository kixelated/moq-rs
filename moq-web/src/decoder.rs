use crate::Result;

pub struct Decoder {
	track: moq_karp::TrackConsumer,
	decoder: web_codecs::VideoDecoder,
}

impl Decoder {
	pub fn new(track: moq_karp::TrackConsumer, decoder: web_codecs::VideoDecoder) -> Self {
		Self { track, decoder }
	}

	pub async fn run(&mut self) -> Result<()> {
		while let Some(frame) = self.track.read().await? {
			let frame = web_codecs::EncodedFrame {
				payload: frame.payload,
				timestamp: web_codecs::Timestamp::from_micros(frame.timestamp.as_micros()),
				keyframe: frame.keyframe,
			};

			self.decoder.decode(frame)?;
		}

		Ok(())
	}
}
