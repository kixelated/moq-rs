mod message;

use std::collections::hash_map as hmap;
use std::time;

use quiche;
use quiche::h3::webtransport;

use crate::{media, transport};

#[derive(Default)]
pub struct Session {
    media: Option<media::Source>,
    streams: transport::Streams, // An easy way of buffering stream data.
    tracks: hmap::HashMap<u32, u64>, // map from track_id to current stream_id
}

impl transport::App for Session {
    // Process any updates to a session.
    fn poll(
        &mut self,
        conn: &mut quiche::Connection,
        session: &mut webtransport::ServerSession,
    ) -> anyhow::Result<()> {
        loop {
            let event = match session.poll(conn) {
                Err(webtransport::Error::Done) => break,
                Err(e) => return Err(e.into()),
                Ok(e) => e,
            };

            match event {
                webtransport::ServerEvent::ConnectRequest(_req) => {
                    // you can handle request with
                    // req.authority()
                    // req.path()
                    // and you can validate this request with req.origin()
                    session.accept_connect_request(conn, None).unwrap();

                    // TODO
                    let media = media::Source::new("../media/fragmented.mp4")?;
                    let init = &media.init;

                    // Create a JSON header.
                    let mut message = message::Message::new();
                    message.init = Some(message::Init {});
                    let data = message.serialize()?;

                    // Create a new stream and write the header.
                    let stream_id = session.open_stream(conn, false)?;
                    self.streams.send(conn, stream_id, data.as_slice(), false)?;
                    self.streams.send(conn, stream_id, init.as_slice(), true)?;

                    self.media = Some(media);
                }
                webtransport::ServerEvent::StreamData(stream_id) => {
                    let mut buf = vec![0; 10000];
                    while let Ok(len) = session.recv_stream_data(conn, stream_id, &mut buf) {
                        let _stream_data = &buf[0..len];
                    }
                }

                _ => {}
            }
        }

        // Send any pending stream data.
        self.streams.poll(conn)?;

        // Fetch the next media fragment, possibly queuing up stream data.
        self.poll_source(conn, session)?;

        Ok(())
    }

    fn timeout(&self) -> Option<time::Duration> {
        self.media.as_ref().and_then(|m| m.timeout())
    }
}

impl Session {
    fn poll_source(
        &mut self,
        conn: &mut quiche::Connection,
        session: &mut webtransport::ServerSession,
    ) -> anyhow::Result<()> {
        // Get the media source once the connection is established.
        let media = match &mut self.media {
            Some(m) => m,
            None => return Ok(()),
        };

        // Get the next media fragment.
        let fragment = match media.fragment()? {
            Some(f) => f,
            None => return Ok(()),
        };

        let stream_id = match self.tracks.get(&fragment.track_id) {
            // Close the old stream.
            Some(stream_id) if fragment.keyframe => {
                self.streams.send(conn, *stream_id, &[], true)?;
                None
            }

            // Use the existing stream
            Some(stream_id) => Some(*stream_id),

            // No existing stream.
            _ => None,
        };

        let stream_id = match stream_id {
            // Use the existing stream,
            Some(stream_id) => stream_id,

            // Open a new stream.
            None => {
                // Create a new unidirectional stream.
                let stream_id = session.open_stream(conn, false)?;

                // Set the stream priority to be equal to the timestamp.
                // We subtract from u64::MAX so newer media is sent important.
                // TODO prioritize audio
                let order = u64::MAX - fragment.timestamp;
                self.streams.send_order(conn, stream_id, order);

                // Encode a JSON header indicating this is a new track.
                let mut message: message::Message = message::Message::new();
                message.segment = Some(message::Segment {
                    track_id: fragment.track_id,
                });

                // Write the header.
                let data = message.serialize()?;
                self.streams.send(conn, stream_id, &data, false)?;

                // Keep a mapping from the track id to the current stream id.
                self.tracks.insert(fragment.track_id, stream_id);

                stream_id
            }
        };

        // Write the current fragment.
        let data = fragment.data.as_slice();
        self.streams.send(conn, stream_id, data, false)?;

        Ok(())
    }
}
