use std::time;

use quiche;
use quiche::h3::webtransport;

use crate::{media,transport};
use super::message;

#[derive(Default)]
pub struct Session {
    media: Option<media::Source>,
    stream_id: Option<u64>, // stream ID of the current segment
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

        self.poll_source(conn, session)?;

        Ok(())
    }

    fn timeout(&self) -> Option<time::Duration> {
        None
    }
}

impl Session {
    fn poll_source(&mut self, conn: &mut quiche::Connection, session: &mut webtransport::ServerSession) -> anyhow::Result<()> {
        let media = match &mut self.media {
            Some(m) => m,
            None => return Ok(()),
        };

        let fragment = match media.next()? {
            Some(f) => f,
            None => return Ok(()),
        };

        log::debug!("{} {}", fragment.keyframe, fragment.timestamp);

        let mut stream_id = match self.stream_id {
            Some(stream_id) => stream_id,
            None => {
                let mut message = message::Message::new();
                message.init = Some(message::Init{
                    id: "video".to_string(),
                });

                let data = message.serialize()?;

                // TODO handle when stream is full
                let stream_id = session.open_stream(conn, false)?;
                session.send_stream_data(conn, stream_id, data.as_slice())?;

                stream_id
            },
        };

        if fragment.keyframe {
            // Close the prior stream.
            conn.stream_send(stream_id, &[], true)?;

            let mut message = message::Message::new();
            message.segment = Some(message::Segment{
                init: "video".to_string(),
                timestamp: fragment.timestamp,
            });

            let data = message.serialize()?;

            // TODO: conn.stream_priority(stream_id, urgency, incremental)

            // TODO handle when stream is full
            stream_id = session.open_stream(conn, false)?;
            session.send_stream_data(conn, stream_id, data.as_slice())?;
        }

        let data = fragment.data.as_slice();

        // TODO check if stream is writable
        session.send_stream_data(conn, stream_id, data)?;

        log::debug!("wrote {} to {}", std::str::from_utf8(&data[4..8]).unwrap(), stream_id);

        // Save for the next fragment
        self.stream_id = Some(stream_id);

        Ok(())
    }
}