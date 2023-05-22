use std::io;

use quiche::h3::webtransport;

use super::app;
use super::connection;

const MAX_DATAGRAM_SIZE: usize = 1350;

pub struct Server<T: app::App> {
    // IO stuff
    socket: mio::net::UdpSocket,
    poll: mio::Poll,
    events: mio::Events,

    // QUIC stuff
    quic: quiche::Config,
    seed: ring::hmac::Key, // connection ID seed

    conns: connection::Map<T>,
}

pub struct Config {
    pub addr: String,
    pub cert: String,
    pub key: String,
}

impl<T: app::App> Server<T> {
    pub fn new(config: Config) -> io::Result<Self> {
        // Listen on the provided socket address
        let addr = config.addr.parse().unwrap();
        let mut socket = mio::net::UdpSocket::bind(addr).unwrap();

        // Setup the event loop.
        let poll = mio::Poll::new().unwrap();
        let events = mio::Events::with_capacity(1024);

        poll.registry()
            .register(&mut socket, mio::Token(0), mio::Interest::READABLE)
            .unwrap();

        // Generate random values for connection IDs.
        let rng = ring::rand::SystemRandom::new();
        let seed = ring::hmac::Key::generate(ring::hmac::HMAC_SHA256, &rng).unwrap();

        // Create the configuration for the QUIC conns.
        let mut quic = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();
        quic.load_cert_chain_from_pem_file(&config.cert).unwrap();
        quic.load_priv_key_from_pem_file(&config.key).unwrap();
        quic.set_application_protos(quiche::h3::APPLICATION_PROTOCOL)
            .unwrap();
        quic.set_max_idle_timeout(5000);
        quic.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
        quic.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
        quic.set_initial_max_data(10_000_000);
        quic.set_initial_max_stream_data_bidi_local(1_000_000);
        quic.set_initial_max_stream_data_bidi_remote(1_000_000);
        quic.set_initial_max_stream_data_uni(1_000_000);
        quic.set_initial_max_streams_bidi(100);
        quic.set_initial_max_streams_uni(100);
        quic.set_disable_active_migration(true);
        quic.enable_early_data();
        quic.enable_dgram(true, 65536, 65536);

        let conns = Default::default();

        Ok(Server {
            socket,
            poll,
            events,

            quic,
            seed,

            conns,
        })
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        log::info!("listening on {}", self.socket.local_addr()?);

        loop {
            self.wait()?;
            self.receive()?;
            self.app()?;
            self.send()?;
            self.cleanup();
        }
    }

    pub fn wait(&mut self) -> anyhow::Result<()> {
        // Find the shorter timeout from all the active connections.
        //
        // TODO: use event loop that properly supports timers
        let timeout = self
            .conns
            .values()
            .filter_map(|c| {
                let timeout = c.quiche.timeout();
                let expires = c.app.timeout();

                match (timeout, expires) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                }
            })
            .min();

        self.poll.poll(&mut self.events, timeout).unwrap();

        // If the event loop reported no events, it means that the timeout
        // has expired, so handle it without attempting to read packets. We
        // will then proceed with the send loop.
        if self.events.is_empty() {
            for conn in self.conns.values_mut() {
                conn.quiche.on_timeout();
            }
        }

        Ok(())
    }

    // Reads packets from the socket, updating any internal connection state.
    fn receive(&mut self) -> anyhow::Result<()> {
        let mut src = [0; MAX_DATAGRAM_SIZE];

        // Try reading any data currently available on the socket.
        loop {
            let (len, from) = match self.socket.recv_from(&mut src) {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) => return Err(e.into()),
            };

            let src = &mut src[..len];

            let info = quiche::RecvInfo {
                to: self.socket.local_addr().unwrap(),
                from,
            };

            // Parse the QUIC packet's header.
            let hdr = quiche::Header::from_slice(src, quiche::MAX_CONN_ID_LEN).unwrap();

            let conn_id = ring::hmac::sign(&self.seed, &hdr.dcid);
            let conn_id = &conn_id.as_ref()[..quiche::MAX_CONN_ID_LEN];
            let conn_id = conn_id.to_vec().into();

            // Check if it's an existing connection.
            if let Some(conn) = self.conns.get_mut(&hdr.dcid) {
                conn.quiche.recv(src, info)?;

                if conn.session.is_none() && conn.quiche.is_established() {
                    conn.session = Some(webtransport::ServerSession::with_transport(
                        &mut conn.quiche,
                    )?)
                }

                continue;
            } else if let Some(conn) = self.conns.get_mut(&conn_id) {
                conn.quiche.recv(src, info)?;

                // TODO is this needed here?
                if conn.session.is_none() && conn.quiche.is_established() {
                    conn.session = Some(webtransport::ServerSession::with_transport(
                        &mut conn.quiche,
                    )?)
                }

                continue;
            }

            if hdr.ty != quiche::Type::Initial {
                log::warn!("unknown connection ID");
                continue;
            }

            let mut dst = [0; MAX_DATAGRAM_SIZE];

            if !quiche::version_is_supported(hdr.version) {
                let len = quiche::negotiate_version(&hdr.scid, &hdr.dcid, &mut dst).unwrap();
                let dst = &dst[..len];

                self.socket.send_to(dst, from).unwrap();
                continue;
            }

            let mut scid = [0; quiche::MAX_CONN_ID_LEN];
            scid.copy_from_slice(&conn_id);

            let scid = quiche::ConnectionId::from_ref(&scid);

            // Token is always present in Initial packets.
            let token = hdr.token.as_ref().unwrap();

            // Do stateless retry if the client didn't send a token.
            if token.is_empty() {
                let new_token = mint_token(&hdr, &from);

                let len = quiche::retry(
                    &hdr.scid,
                    &hdr.dcid,
                    &scid,
                    &new_token,
                    hdr.version,
                    &mut dst,
                )
                .unwrap();

                let dst = &dst[..len];

                self.socket.send_to(dst, from).unwrap();
                continue;
            }

            let odcid = validate_token(&from, token);

            // The token was not valid, meaning the retry failed, so
            // drop the packet.
            if odcid.is_none() {
                log::warn!("invalid token");
                continue;
            }

            if scid.len() != hdr.dcid.len() {
                log::warn!("invalid connection ID");
                continue;
            }

            // Reuse the source connection ID we sent in the Retry packet,
            // instead of changing it again.
            let conn_id = hdr.dcid.clone();
            let local_addr = self.socket.local_addr().unwrap();

            log::debug!("new connection: dcid={:?} scid={:?}", hdr.dcid, scid);

            let mut conn =
                quiche::accept(&conn_id, odcid.as_ref(), local_addr, from, &mut self.quic)?;

            // Process potentially coalesced packets.
            conn.recv(src, info)?;

            let user = connection::Connection {
                quiche: conn,
                session: None,
                app: T::default(),
            };

            self.conns.insert(conn_id, user);
        }
    }

    pub fn app(&mut self) -> anyhow::Result<()> {
        for conn in self.conns.values_mut() {
            if conn.quiche.is_closed() {
                continue;
            }

            if let Some(session) = &mut conn.session {
                if let Err(e) = conn.app.poll(&mut conn.quiche, session) {
                    log::debug!("app error: {:?}", e);

                    // Close the connection on any application error
                    let reason = format!("app error: {:?}", e);
                    conn.quiche.close(true, 0xff, reason.as_bytes()).ok();
                }
            }
        }

        Ok(())
    }

    // Generate outgoing QUIC packets for all active connections and send
    // them on the UDP socket, until quiche reports that there are no more
    // packets to be sent.
    pub fn send(&mut self) -> anyhow::Result<()> {
        for conn in self.conns.values_mut() {
            let conn = &mut conn.quiche;

            if let Err(e) = send_conn(&self.socket, conn) {
                log::error!("{} send failed: {:?}", conn.trace_id(), e);
                conn.close(false, 0x1, b"fail").ok();
            }
        }

        Ok(())
    }

    pub fn cleanup(&mut self) {
        // Garbage collect closed connections.
        self.conns.retain(|_, ref mut c| !c.quiche.is_closed());
    }
}

// Send any pending packets for the connection over the socket.
fn send_conn(socket: &mio::net::UdpSocket, conn: &mut quiche::Connection) -> anyhow::Result<()> {
    let mut pkt = [0; MAX_DATAGRAM_SIZE];

    loop {
        let (size, info) = match conn.send(&mut pkt) {
            Ok(v) => v,
            Err(quiche::Error::Done) => return Ok(()),
            Err(e) => return Err(e.into()),
        };

        let pkt = &pkt[..size];

        match socket.send_to(pkt, info.to) {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(e) => return Err(e.into()),
            Ok(_) => (),
        }
    }
}

/// Generate a stateless retry token.
///
/// The token includes the static string `"quiche"` followed by the IP address
/// of the client and by the original destination connection ID generated by the
/// client.
///
/// Note that this function is only an example and doesn't do any cryptographic
/// authenticate of the token. *It should not be used in production system*.
fn mint_token(hdr: &quiche::Header, src: &std::net::SocketAddr) -> Vec<u8> {
    let mut token = Vec::new();

    token.extend_from_slice(b"quiche");

    let addr = match src.ip() {
        std::net::IpAddr::V4(a) => a.octets().to_vec(),
        std::net::IpAddr::V6(a) => a.octets().to_vec(),
    };

    token.extend_from_slice(&addr);
    token.extend_from_slice(&hdr.dcid);

    token
}

/// Validates a stateless retry token.
///
/// This checks that the ticket includes the `"quiche"` static string, and that
/// the client IP address matches the address stored in the ticket.
///
/// Note that this function is only an example and doesn't do any cryptographic
/// authenticate of the token. *It should not be used in production system*.
fn validate_token<'a>(
    src: &std::net::SocketAddr,
    token: &'a [u8],
) -> Option<quiche::ConnectionId<'a>> {
    if token.len() < 6 {
        return None;
    }

    if &token[..6] != b"quiche" {
        return None;
    }

    let token = &token[6..];

    let addr = match src.ip() {
        std::net::IpAddr::V4(a) => a.octets().to_vec(),
        std::net::IpAddr::V6(a) => a.octets().to_vec(),
    };

    if token.len() < addr.len() || &token[..addr.len()] != addr.as_slice() {
        return None;
    }

    Some(quiche::ConnectionId::from_ref(&token[addr.len()..]))
}
