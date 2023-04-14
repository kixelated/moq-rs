use crate::error;
use error::Result;

use std::collections::HashMap;
use quiche::h3::webtransport;

pub struct Session {
    pub conn: quiche::Connection,
    pub session: Option<webtransport::ServerSession>,
}

pub type Map = HashMap<quiche::ConnectionId<'static>, Session>;

impl Session {
    // Process any updates to a session.
    pub fn poll(&mut self) -> Result<()> {
        let session = match &mut self.session {
            Some(s) => s,
            None => return Ok(()),
        };

        loop {
            let event = match session.poll(&mut self.conn) {
                Err(webtransport::Error::Done) => return Ok(()),
                Err(e) => return Err(e.into()),
                Ok(e) => e,
            };

            match event {
                webtransport::ServerEvent::ConnectRequest(_req) => {
                    // you can handle request with
                    // req.authority()
                    // req.path()
                    // and you can validate this request with req.origin()
                    session.accept_connect_request(&mut self.conn, None).unwrap();
                },
                webtransport::ServerEvent::StreamData(stream_id) => {
                    let mut buf = vec![0; 10000];
                    while let Ok(len) =
                        session.recv_stream_data(&mut self.conn, stream_id, &mut buf)
                    {
                        let stream_data = &buf[0..len];

                        // handle stream_data
                        if (stream_id & 0x2) == 0 {
                            // bidirectional stream
                            // you can send data through this stream.
                            session
                                .send_stream_data(&mut self.conn, stream_id, stream_data)
                                .unwrap();
                        } else {
                            // you cannot send data through client-initiated-unidirectional-stream.
                            // so, open new server-initiated-unidirectional-stream, and send data
                            // through it.
                            let new_stream_id =
                                session.open_stream(&mut self.conn, false).unwrap();
                            session
                                .send_stream_data(&mut self.conn, new_stream_id, stream_data)
                                .unwrap();
                        }
                    }
                }

                webtransport::ServerEvent::StreamFinished(_stream_id) => {
                    // A WebTrnasport stream finished, handle it.
                }

                webtransport::ServerEvent::Datagram => {
                    let mut buf = vec![0; 1500];
                    while let Ok((in_session, offset, total)) =
                        session.recv_dgram(&mut self.conn, &mut buf)
                    {
                        if in_session {
                            let dgram = &buf[offset..total];
                            dbg!(std::string::String::from_utf8_lossy(dgram));
                            // handle this dgram

                            // for instance, you can write echo-server like following
                            session.send_dgram(&mut self.conn, dgram).unwrap();
                        } else {
                            // this dgram is not related to current WebTransport session. ignore.
                        }
                    }
                }

                webtransport::ServerEvent::SessionReset(_e) => {
                    // Peer reset session stream, handle it.
                }

                webtransport::ServerEvent::SessionFinished => {
                    // Peer finish session stream, handle it.
                }

                webtransport::ServerEvent::SessionGoAway => {
                    // Peer signalled it is going away, handle it.
                }

                webtransport::ServerEvent::Other(_stream_id, _event) => {
                    // Original h3::Event which is not related to WebTransport.
                }
            }
        }
    }
}