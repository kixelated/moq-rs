use web_async::FuturesExt;

use crate::Result;

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
