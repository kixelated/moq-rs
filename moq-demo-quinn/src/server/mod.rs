use std::{fs, io, sync::{self, Arc}, time};

use anyhow::Context;
use bytes::Bytes;
use h3::{quic::StreamId, proto::varint::VarInt};
use h3_webtransport::server::AcceptedBi;
use moq_transport::Message;
use moq_transport_generic::{AcceptSetup, Control, Objects};
use moq_warp::relay::{broker, ServerConfig};
use tokio::task::JoinSet;
use warp::{Future, http};

use self::stream::{QuinnBidiStream, QuinnSendStream, QuinnRecvStream};

mod stream;

fn stream_id_to_u64(stream_id: StreamId) -> u64 {
	let val: VarInt = stream_id.into();
	val.into_inner()
}

pub struct Server {
	// The MoQ transport server.
	server: h3_webtransport::server::WebTransportSession<h3_quinn::Connection, bytes::Bytes>,
}

impl Server {
	// Create a new server
	pub fn new_quinn_connection(config: ServerConfig) -> anyhow::Result<h3_quinn::Endpoint> {
		// Read the PEM certificate chain
		let certs = fs::File::open(config.cert).context("failed to open cert file")?;
		let mut certs = io::BufReader::new(certs);
		let certs = rustls_pemfile::certs(&mut certs)?
			.into_iter()
			.map(rustls::Certificate)
			.collect();

		// Read the PEM private key
		let keys = fs::File::open(config.key).context("failed to open key file")?;
		let mut keys = io::BufReader::new(keys);
		let mut keys = rustls_pemfile::pkcs8_private_keys(&mut keys)?;

		anyhow::ensure!(keys.len() == 1, "expected a single key");
		let key = rustls::PrivateKey(keys.remove(0));

		let mut tls_config = rustls::ServerConfig::builder()
			.with_safe_default_cipher_suites()
			.with_safe_default_kx_groups()
			.with_protocol_versions(&[&rustls::version::TLS13])
			.unwrap()
			.with_no_client_auth()
			.with_single_cert(certs, key)?;

		tls_config.max_early_data_size = u32::MAX;
		let alpn: Vec<Vec<u8>> = vec![
			b"h3".to_vec(),
			b"h3-32".to_vec(),
			b"h3-31".to_vec(),
			b"h3-30".to_vec(),
			b"h3-29".to_vec(),
		];
		tls_config.alpn_protocols = alpn;

		let mut server_config = quinn::ServerConfig::with_crypto(sync::Arc::new(tls_config));

		// Enable BBR congestion control
		// TODO validate the implementation
		let mut transport_config = quinn::TransportConfig::default();
		transport_config.keep_alive_interval(Some(time::Duration::from_secs(2)));
		transport_config.congestion_controller_factory(sync::Arc::new(quinn::congestion::BbrConfig::default()));

		server_config.transport = sync::Arc::new(transport_config);
		let server = quinn::Endpoint::server(server_config, config.addr)?;

		Ok(server)
	}

	pub async fn accept_new_webtransport_session(endpoint: &h3_quinn::Endpoint) -> anyhow::Result<Connect> {
		let mut handshake = JoinSet::new();
		// perform a quic handshake
		loop {
			tokio::select!(
				// Accept the connection and start the WebTransport handshake.
				conn = endpoint.accept() => {
					let conn = conn.context("failed to accept connection").unwrap();
					handshake.spawn(async move {
						
						let conn = conn.await.context("failed to accept h3 connection")?;

						let mut conn = h3::server::builder()
							.enable_webtransport(true)
							.enable_connect(true)
							.enable_datagram(true)
							.max_webtransport_sessions(1)
							.send_grease(true)
							.build(h3_quinn::Connection::new(conn))
							.await
							.context("failed to create h3 server")?;

						let (req, stream) = conn
							.accept()
							.await
							.context("failed to accept h3 session")?
							.context("failed to accept h3 request")?;

						let ext = req.extensions();
						anyhow::ensure!(req.method() == http::Method::CONNECT, "expected CONNECT request");
						anyhow::ensure!(
							ext.get::<h3::ext::Protocol>() == Some(&h3::ext::Protocol::WEB_TRANSPORT),
							"expected WebTransport CONNECT"
						);

						// Let the application decide if we accept this CONNECT request.
						Ok(Connect { conn, req, stream })
					});
				},
				// Return any mostly finished WebTransport handshakes.
				res = handshake.join_next(), if !handshake.is_empty() => {
					let res = res.expect("no tasks").expect("task aborted");
					match res {
						Ok(connect_request) => return Ok(connect_request),
						Err(err) => log::warn!("failed to accept session: {:?}", err),
					}
				},
			)
		}
	}

	// pub async fn run(mut self) -> anyhow::Result<()> {
	// 	loop {
	// 		tokio::select! {
	// 			res = self.server.accept() => {
	// 				let session = res.context("failed to accept connection")?;
	// 				let broker = self.broker.clone();

	// 				self.tasks.spawn(async move {
	// 					let session: Session = Session::accept(session, broker).await?;
	// 					session.run().await
	// 				});
	// 			},
				// res = self.tasks.join_next(), if !self.tasks.is_empty() => {
				// 	let res = res.expect("no tasks").expect("task aborted");

				// 	if let Err(err) = res {
				// 		log::error!("session terminated: {:?}", err);
				// 	}
				// },
	// 		}
	// 	}
	// }
}

// The WebTransport CONNECT has arrived, and we need to decide if we accept it.
pub struct Connect {
	// Inspect to decide whether to accept() or reject() the session.
	req: http::Request<()>,

	conn: h3::server::Connection<h3_quinn::Connection, Bytes>,
	stream: h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
}

impl Connect {
	// Expose the received URI
	pub fn uri(&self) -> &http::Uri {
		self.req.uri()
	}

	// Accept the WebTransport session.
	pub async fn accept(self) -> anyhow::Result<AcceptSetup<Server>> {
		let session = h3_webtransport::server::WebTransportSession::accept(self.req, self.stream, self.conn).await?;
		let mut session = Server{server: session};

		let control_stream = moq_generic_transport::accept_bidi(&mut session)
			.await
			.context("failed to accept bidi stream")?
			.unwrap();

		Ok(moq_transport_generic::Session::accept(Box::new(control_stream), Box::new(session)).await?)
	}

	// Reject the WebTransport session with a HTTP response.
	pub async fn reject(mut self, resp: http::Response<()>) -> anyhow::Result<()> {
		self.stream.send_response(resp).await?;
		Ok(())
	}
}


impl moq_generic_transport::Connection for Server {
    type BidiStream = QuinnBidiStream;

    type SendStream = QuinnSendStream;

    type RecvStream = QuinnRecvStream;

    fn poll_accept_recv(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Option<Self::RecvStream>, anyhow::Error>> {
		let fut = self.server.accept_uni();
		let fut = std::pin::pin!(fut);
		fut.poll(cx)
		.map_ok(|opt| opt.map(|(_, s)| QuinnRecvStream::new(s)))
		.map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    fn poll_accept_bidi(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Option<Self::BidiStream>, anyhow::Error>> {
		let fut = self.server.accept_bi();
		let fut = std::pin::pin!(fut);
		let res = std::task::ready!(fut.poll(cx).map_err(|e| anyhow::anyhow!("{:?}", e)));
		match res {
			Ok(Some(AcceptedBi::Request(_, _))) => std::task::Poll::Ready(Err(anyhow::anyhow!("received new session whils accepting bidi stream"))),
			Ok(Some(AcceptedBi::BidiStream(_, s))) => std::task::Poll::Ready(Ok(Some(QuinnBidiStream::new(s)))),
			Ok(None) => std::task::Poll::Ready(Ok(None)),
			Err(e) => std::task::Poll::Ready(Err(e)),
		}
    }

    fn poll_open_bidi(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Self::BidiStream, anyhow::Error>> {
		let fut = self.server.open_bi(self.server.session_id());
		let fut = std::pin::pin!(fut);
		fut.poll(cx)
		.map_ok(|s| QuinnBidiStream::new(s))
		.map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    fn poll_open_send(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Self::SendStream, anyhow::Error>> {
		let fut = self.server.open_uni(self.server.session_id());
		let fut = std::pin::pin!(fut);
		fut.poll(cx)
		.map_ok(|s| QuinnSendStream::new(s))
		.map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    fn close(&mut self, _code: u64, _reason: &[u8]) {
        todo!("not implemented")
    }
}