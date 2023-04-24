use std::collections::hash_map as hmap;
use quiche::h3::webtransport;

type Session = webtransport::ServerSession;
type Map = hmap::HashMap<quiche::ConnectionId<'static>, Session>;

/*
impl Session {
    pub fn with_transport(conn: &mut quiche::Connection) -> anyhow::Result<Self> {
        let session = webtransport::ServerSession::with_transport(conn)?;

        Ok(Self{
            session
        })
    }

    // Process any updates to a session.
    pub fn poll(&mut self) -> anyhow::Result<()> {
        log::debug!("poll conn");
        while self.poll_once()? {}

        log::debug!("poll streams");
        self.poll_streams()?;

        Ok(())
    }

    // Process any updates to a session.
    pub fn poll_once(&mut self) -> anyhow::Result<bool> {
        let session = match &mut self.session {
            Some(s) => s,
            None => return Ok(false),
        };

        let event = match session.poll(&mut self.conn) {
            Err(webtransport::Error::Done) => return Ok(false),
            Err(e) => return Err(e.into()),
            Ok(e) => e,
        };

        match event {
            webtransport::ServerEvent::ConnectRequest(req) => {
                log::debug!("new connect {:?}", req);
                // you can handle request with
                // req.authority()
                // req.path()
                // and you can validate this request with req.origin()
                session.accept_connect_request(&mut self.conn, None).unwrap();
            },
            webtransport::ServerEvent::StreamData(stream_id) => {
                log::debug!("on stream data {}", stream_id);

                let mut buf = vec![0; 10000];
                while let Ok(len) =
                    session.recv_stream_data(&mut self.conn, stream_id, &mut buf)
                {
                    let stream_data = &buf[0..len];
                    log::debug!("stream data {:?}", stream_data);

/*
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
                    */
                }
            }

            webtransport::ServerEvent::StreamFinished(stream_id) => {
                // A WebTrnasport stream finished, handle it.
                log::debug!("stream finished {}", stream_id);
            }

            webtransport::ServerEvent::Datagram => {
                log::debug!("datagram");
            }

            webtransport::ServerEvent::SessionReset(e) => {
                log::debug!("session reset {}", e);
                // Peer reset session stream, handle it.
            }

            webtransport::ServerEvent::SessionFinished => {
                log::debug!("session finished");
                // Peer finish session stream, handle it.
            }

            webtransport::ServerEvent::SessionGoAway => {
                log::debug!("session go away");
                // Peer signalled it is going away, handle it.
            }

            webtransport::ServerEvent::Other(stream_id, event) => {
                log::debug!("session other: {} {:?}", stream_id, event);
                // Original h3::Event which is not related to WebTransport.
            }
        }

        Ok(true)
    }

/*
    fn poll_source(&mut self) -> anyhow::Result<()> {
        let media = match &mut self.media {
            Some(m) => m,
            None => return Ok(()),
        };

        let fragment = match media.next()? {
            Some(f) => f,
            None => return Ok(()),
        };

        // Get or create a new stream for each unique segment ID.
        let stream_id = match self.segments.entry(fragment.segment_id) {
            map::Entry::Occupied(e) => e.into_mut(),
            map::Entry::Vacant(e) => {
                let stream_id = self.start_stream(&fragment)?;
                e.insert(stream_id)
            },
        };

        // Get or create a buffered object for each unique stream ID.
        let buffered = match self.streams.entry(*stream_id) {
            map::Entry::Occupied(e) => e.into_mut(),
            map::Entry::Vacant(e) => e.insert(Buffered::new()),
        };

        let session = match &mut self.session {
            Some(s) => s,
            None => return Ok(()),
        };

        let data = fragment.data.as_slice();

        match self.conn.stream_writable(*stream_id, data.len()) {
            Ok(true) if buffered.len() == 0 => {
                session.send_stream_data(&mut self.conn, *stream_id, data)?;
            },
            Ok(_) => buffered.push_back(fragment.data),
            Err(quiche::Error::Done) => {}, // stream closed?
            Err(e) => anyhow::bail!(e),
        };

        Ok(())
    }

    fn start_stream(&mut self, fragment: &source::Fragment) -> anyhow::Result<u64> {
        let conn = &mut self.conn;
        let session = self.session.as_mut().unwrap();

        let stream_id = session.open_stream(conn, false)?;

        // TODO: conn.stream_priority(stream_id, urgency, incremental)

        let mut message = message::Message::new();
        if fragment.segment_id == 0 {
            message.init = Some(message::Init{
                id: "video".to_string(),
            });
        } else {
            message.segment = Some(message::Segment{
                init: "video".to_string(),
                timestamp: fragment.timestamp,
            });
        }

        let data= message.serialize()?;
        match conn.stream_writable(stream_id, data.len()) {
            Ok(true) => {
                session.send_stream_data(conn, stream_id, data.as_slice())?;
            },
            Ok(false) => {
                let mut buffered = Buffered::new();
                buffered.push_back(data);

                self.streams.insert(stream_id, buffered);
            },
            Err(quiche::Error::Done) => {},
            Err(e) => anyhow::bail!(e),
        };

        Ok(stream_id)
    }
*/

    fn poll_streams(&mut self) -> anyhow::Result<()> {
        // TODO make sure this loops in priority order
        for stream_id in self.conn.writable() {
            self.poll_stream(stream_id)?;
        }

        // Remove any entry buffered values.
        self.streams.retain(|_, buffered| buffered.len() > 0 );

        Ok(())
    }

    pub fn poll_stream(&mut self, stream_id: u64) -> anyhow::Result<()> {
        let buffered = match self.streams.get_mut(&stream_id) {
            Some(b) => b,
            None => return Ok(()),
        };

        let conn = &mut self.conn;

        let session = match &mut self.session {
            Some(s) => s,
            None => return Ok(()),
        };

        while let Some(data) = buffered.pop_front() {
            match conn.stream_writable(stream_id, data.len()) {
                Ok(true) => {
                    session.send_stream_data(conn, stream_id, data.as_slice())?;
                },
                Ok(false) => {
                    buffered.push_front(data);
                    return Ok(());
                },
                Err(quiche::Error::Done) => {},
                Err(e) => anyhow::bail!(e),
            };
        }

        Ok(())
    }

    pub fn timeout(&self) -> Option<time::Duration> {
        self.conn.timeout()
    }

    pub fn on_timeout(&mut self) {
       self.conn.on_timeout()

       // custom stuff here
    }
}
*/