use std::time;

use quiche;
use quiche::h3::webtransport;

use crate::{media,transport};
use super::message;

#[derive(Default)]
pub struct Session {
    media: Option<media::Source>,
    stream_id: Option<u64>, // stream ID of the current segment

    streams: transport::Streams, // An easy way of buffering stream data.
}

impl transport::App for Session {
    // Process any updates to a session.
    fn poll(&mut self, conn: &mut quiche::Connection, session: &mut webtransport::ServerSession) -> anyhow::Result<()> {
        loop {
            let event = match session.poll(conn) {
                Err(webtransport::Error::Done) => break,
                Err(e) => return Err(e.into()),
                Ok(e) => e,
            };

            log::debug!("webtransport event: {:?}", event);

            match event {
                webtransport::ServerEvent::ConnectRequest(req) => {
                    log::debug!("new connect {:?}", req);
                    // you can handle request with
                    // req.authority()
                    // req.path()
                    // and you can validate this request with req.origin()

                    // TODO
                    let media = media::Source::new("../media/fragmented.mp4")?;
                    self.media = Some(media);

                    session.accept_connect_request(conn, None).unwrap();
                },
                webtransport::ServerEvent::StreamData(stream_id) => {
                    let mut buf = vec![0; 10000];
                    while let Ok(len) =
                        session.recv_stream_data(conn, stream_id, &mut buf)
                    {
                        let stream_data = &buf[0..len];
                        log::debug!("stream data {:?}", stream_data);
                    }
                }

                _ => {},
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
    fn poll_source(&mut self, conn: &mut quiche::Connection, session: &mut webtransport::ServerSession) -> anyhow::Result<()> {
        // Get the media source once the connection is established.
        let media = match &mut self.media {
            Some(m) => m,
            None => return Ok(()),
        };

        // Get the next media fragment.
        let fragment = match media.next()? {
            Some(f) => f,
            None => return Ok(()),
        };

        // Check if we have already created a stream for this fragment.
        let stream_id = match self.stream_id {
            Some(old_stream_id) if fragment.keyframe => {
                // This is the start of a new segment.

                // Close the prior stream.
                self.streams.send(conn, old_stream_id, &[], true)?;

                // Encode a JSON header indicating this is the video track.
                let mut message = message::Message::new();
                message.segment = Some(message::Segment{
                    init: "video".to_string(),
                });

                // Open a new stream.
                let stream_id = session.open_stream(conn, false)?;
                // TODO: conn.stream_priority(stream_id, urgency, incremental)

                // Write the header.
                let data = message.serialize()?;
                self.streams.send(conn, stream_id, &data, false)?;

                stream_id
            },
            None => {
                // This is the start of an init segment.

                // Create a JSON header.
                let mut message = message::Message::new();
                message.init = Some(message::Init{
                    id: "video".to_string(),
                });

                let data = message.serialize()?;

                // Create a new stream and write the header.
                let stream_id = session.open_stream(conn, false)?;
                self.streams.send(conn, stream_id, data.as_slice(), false)?;

                stream_id
            }
            Some(stream_id) => stream_id, // Continuation of init or segment
        };

        // Write the current fragment.
        let data = fragment.data.as_slice();
        self.streams.send(conn, stream_id, data, false)?;

        // Save the stream ID for the next fragment.
        self.stream_id = Some(stream_id);

        Ok(())
    }
}