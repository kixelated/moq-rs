use std::time;

use quiche;
use quiche::h3::webtransport;

use crate::{media,transport};
use super::message;

use mp4;

#[derive(Default)]
pub struct Session {
    media: Option<media::Source>,
    stream_id: Option<u64>, // stream ID of the current segment
    styp: Option<Vec<u8>>,
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
        self.media.as_ref().and_then(|m| m.timeout())
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
            });

            let data = message.serialize()?;

            // TODO: conn.stream_priority(stream_id, urgency, incremental)

            // TODO handle when stream is full
            stream_id = session.open_stream(conn, false)?;
            session.send_stream_data(conn, stream_id, data.as_slice())?;

            let styp = self.styp.as_ref().expect("missing ftyp mox");
            session.send_stream_data(conn, stream_id, &styp)?;
        }

        let data = fragment.data.as_slice();

        // TODO check if stream is writable
        let size = session.send_stream_data(conn, stream_id, data)?;
        if size < data.len() {
            anyhow::bail!("partial write: {} < {}", size, data.len());
        }

        // Save for the next fragment
        self.stream_id = Some(stream_id);

        // Save the ftyp fragment but modify it to be a styp for furture segments.
        if fragment.typ == mp4::BoxType::FtypBox {
            let mut data = fragment.data;
            data[4] = b's'; // ftyp to styp

            self.styp = Some(data);
        }

        Ok(())
    }
}