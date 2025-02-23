use moq_async::FuturesExt;

use crate::Result;

pub struct Video {
	pub track: moq_karp::TrackConsumer,

	decoder: web_codecs::VideoDecoder,
	decoded: web_codecs::VideoDecoded,
}

impl Video {
	pub fn new(track: moq_karp::TrackConsumer, info: moq_karp::Video) -> Result<Self> {
		// Construct the video decoder
		let (decoder, decoded) = web_codecs::VideoDecoderConfig {
			codec: info.codec.to_string(),
			description: info.description.clone(),
			resolution: Some(web_codecs::Dimensions {
				width: info.resolution.width,
				height: info.resolution.height,
			}),
			latency_optimized: Some(true),
			..Default::default()
		}
		.build()?;

		Ok(Self {
			track,
			decoder,
			decoded,
		})
	}

	pub async fn frame(&mut self) -> Result<Option<web_codecs::VideoFrame>> {
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
