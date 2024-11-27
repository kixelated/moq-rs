use crate::{Result, Run};

pub struct Decoder {
    track: moq_karp::TrackConsumer,
    decoder: web_codecs::VideoDecoder,
}

impl Decoder {
    pub fn new(track: moq_karp::TrackConsumer, decoder: web_codecs::VideoDecoder) -> Self {
        Self { track, decoder }
    }
}

impl Run for Decoder {
    async fn run(&mut self) -> Result<()> {
        while let Some(frame) = self.track.read().await? {
            let frame = web_codecs::EncodedFrame {
                payload: frame.payload,
                timestamp: frame.timestamp.as_micros() as _,
                keyframe: frame.keyframe,
            };
            self.decoder.decode(frame)?;
        }

        Ok(())
    }
}
