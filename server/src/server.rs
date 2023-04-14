use crate::session;
use crate::error;

use session::Session;
use error::{Error, Result};

use std::{io, net};
use log;

use quiche::h3::webtransport;

const MAX_DATAGRAM_SIZE: usize = 1350;

pub struct Server {
    // IO stuff
    socket: mio::net::UdpSocket,
    poll: mio::Poll,
    events: mio::Events,

    // QUIC stuff
    quic: quiche::Config,
    sessions: session::Map,
    seed: ring::hmac::Key, // connection ID seed
}

pub struct Config {
    pub addr: String,
    pub cert: String,
    pub key: String,
}

impl Server {
    pub fn new(config: Config) -> io::Result<Server> {
        // Listen on the provided socket address
        let addr = config.addr.parse().unwrap();
        let mut socket = mio::net::UdpSocket::bind(addr).unwrap();

        // Setup the event loop.
        let poll = mio::Poll::new().unwrap();
        let events = mio::Events::with_capacity(1024);
        let sessions = session::Map::new();

        poll.registry().register(
            &mut socket,
            mio::Token(0),
            mio::Interest::READABLE,
        ).unwrap();

        // Generate random values for connection IDs.
        let rng = ring::rand::SystemRandom::new();
        let seed = ring::hmac::Key::generate(ring::hmac::HMAC_SHA256, &rng).unwrap();

        // Create the configuration for the QUIC connections.
        let mut quic = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();
        quic.load_cert_chain_from_pem_file(&config.cert).unwrap();
        quic.load_priv_key_from_pem_file(&config.key).unwrap();
        quic.set_application_protos(quiche::h3::APPLICATION_PROTOCOL).unwrap();
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

        Ok(Server {
            socket,
            poll,
            events,

            quic,
            sessions,
            seed
        })
    }

    pub fn poll(&mut self) -> io::Result<()> {
        self.receive().unwrap();
        self.send().unwrap();
        self.cleanup().unwrap();

        Ok(())
    }

    fn receive(&mut self) -> io::Result<()> {
        // Find the shorter timeout from all the active connections.
        //
        // TODO: use event loop that properly supports timers
        let timeout = self.sessions.values().filter_map(|c| c.conn.timeout()).min();

        self.poll.poll(&mut self.events, timeout).unwrap();

        // If the event loop reported no events, it means that the timeout
        // has expired, so handle it without attempting to read packets. We
        // will then proceed with the send loop.
        if self.events.is_empty() {
            self.sessions.values_mut().for_each(|session| {
                session.conn.on_timeout()
            });

            return Ok(())
        }

        // Read incoming UDP packets from the socket and feed them to quiche,
        // until there are no more packets to read.
        loop {
            match self.receive_once() {
                Err(Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) => log::error!("{:?}", e),
                Ok(_) => (),
            }
        }
    }

    fn receive_once(&mut self) -> Result<()> {
        let mut src= [0; MAX_DATAGRAM_SIZE];

        let (len, from) = self.socket.recv_from(&mut src).unwrap();
        let src = &mut src[..len];

        let info = quiche::RecvInfo {
            to: self.socket.local_addr().unwrap(),
            from,
        };

        // Lookup a connection based on the packet's connection ID. If there
        // is no connection matching, create a new one.
        let pair = match self.accept(src, from).unwrap() {
            Some(v) => v,
            None => return Ok(()),
        };

        let conn = &mut pair.conn;

        // Process potentially coalesced packets.
        conn.recv(src, info).unwrap();

        // Create a new HTTP/3 connection as soon as the QUIC connection
        // is established.
        if (conn.is_in_early_data() || conn.is_established()) && pair.session.is_none() {
            let session = webtransport::ServerSession::with_transport(conn).unwrap();
            pair.session = Some(session);
        }

        // The `poll` can pull out the events that occurred according to the data passed here.
        for (_, session) in self.sessions.iter_mut() {
            session.poll().unwrap();
        }

        Ok(())
    }

    fn accept(&mut self, src: &mut [u8], from: net::SocketAddr) -> error::Result<Option<&mut Session>> {
        // Parse the QUIC packet's header.
        let hdr = quiche::Header::from_slice(src, quiche::MAX_CONN_ID_LEN).unwrap();

        let conn_id = ring::hmac::sign(&self.seed, &hdr.dcid);
        let conn_id = &conn_id.as_ref()[..quiche::MAX_CONN_ID_LEN];
        let conn_id = conn_id.to_vec().into();

        if self.sessions.contains_key(&hdr.dcid) {
            let pair = self.sessions.get_mut(&hdr.dcid).unwrap();
            return Ok(Some(pair))
        } else if self.sessions.contains_key(&conn_id) {
            let pair = self.sessions.get_mut(&conn_id).unwrap();
            return Ok(Some(pair));
        }

        if hdr.ty != quiche::Type::Initial {
            return Err(error::Server::UnknownConnectionID.into())
        }

        let mut dst = [0; MAX_DATAGRAM_SIZE];

        if !quiche::version_is_supported(hdr.version) {
            let len = quiche::negotiate_version(&hdr.scid, &hdr.dcid, &mut dst).unwrap();
            let dst= &dst[..len];

            self.socket.send_to(dst, from).unwrap();
            return Ok(None)
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

            let dst= &dst[..len];

            self.socket.send_to(dst, from).unwrap();
            return Ok(None)
        }

        let odcid = validate_token(&from, token);

        // The token was not valid, meaning the retry failed, so
        // drop the packet.
        if odcid.is_none() {
            return Err(error::Server::InvalidToken.into())
        }

        if scid.len() != hdr.dcid.len() {
            return Err(error::Server::InvalidConnectionID.into())
        }

        // Reuse the source connection ID we sent in the Retry packet,
        // instead of changing it again.
        let conn_id= hdr.dcid.clone();
        let local_addr = self.socket.local_addr().unwrap();

        let conn =
            quiche::accept(&conn_id, odcid.as_ref(), local_addr, from, &mut self.quic)
                .unwrap();

        self.sessions.insert(
            conn_id.clone(),
            Session {
                conn,
                session: None,
            },
        );

        let pair = self.sessions.get_mut(&conn_id).unwrap();
        Ok(Some(pair))
    }

    fn send(&mut self) -> io::Result<()> {
        let mut pkt = [0; MAX_DATAGRAM_SIZE];

        // Generate outgoing QUIC packets for all active connections and send
        // them on the UDP socket, until quiche reports that there are no more
        // packets to be sent.
        for session in self.sessions.values_mut() {
            loop {
                let (size , info) = session.conn.send(&mut pkt).unwrap();
                let pkt = &pkt[..size];

                match self.socket.send_to(&pkt, info.to) {
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                    Err(err) => return Err(err),
                    Ok(_) => (),
                }
            }
        }

        Ok(())
    }

    fn cleanup(&mut self) -> io::Result<()> {
        // Garbage collect closed connections.
        self.sessions.retain(|_, session| !session.conn.is_closed() );
        Ok(())
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
    src: &std::net::SocketAddr, token: &'a [u8],
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