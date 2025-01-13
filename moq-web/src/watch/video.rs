use moq_async::FuturesExt;

use crate::Result;

pub struct Video {
	info: moq_karp::Video,
	consumer: moq_karp::TrackConsumer,

	decoder: web_codecs::VideoDecoder,
	decoded: web_codecs::VideoDecoded,
}

impl Video {
	pub fn new(info: moq_karp::Video, consumer: moq_karp::TrackConsumer) -> Result<Self> {
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
			info,
			consumer,

			decoder,
			decoded,
		})
	}

	pub fn info(&self) -> &moq_karp::Video {
		&self.info
	}

	pub fn switch(&mut self, info: moq_karp::Video, consumer: moq_karp::TrackConsumer) {
		self.info = info;
		self.consumer = consumer;
	}

	pub async fn frame(&mut self) -> Result<Option<web_codecs::VideoFrame>> {
		loop {
			tokio::select! {
				Some(frame) = self.consumer.read().transpose() => {
					let frame = frame?;

					let frame = web_codecs::EncodedFrame {
						payload: frame.payload,
						timestamp: web_codecs::Timestamp::from_micros(frame.timestamp.as_micros()),
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
