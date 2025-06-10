use std::path::PathBuf;
use std::{net, sync::Arc, time::Duration};

use anyhow::Context;
use ring::digest::{digest, SHA256};
use rustls::crypto::ring::sign::any_supported_type;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use std::fs;
use std::io::{self, Cursor, Read};
use url::Url;

use futures::future::BoxFuture;
use futures::stream::{FuturesUnordered, StreamExt};
use futures::FutureExt;

use web_transport::quinn as web_transport_quinn;

#[derive(clap::Args, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerTlsCert {
	pub chain: PathBuf,
	pub key: PathBuf,
}

impl ServerTlsCert {
	// A crude colon separated string parser just for clap support.
	pub fn parse(s: &str) -> anyhow::Result<Self> {
		let (chain, key) = s.split_once(':').context("invalid certificate")?;
		Ok(Self {
			chain: PathBuf::from(chain),
			key: PathBuf::from(key),
		})
	}
}

#[derive(clap::Args, Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerTlsConfig {
	/// Load the given certificate and keys from disk.
	#[arg(long = "tls-cert", value_parser = ServerTlsCert::parse)]
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub cert: Vec<ServerTlsCert>,

	/// Or generate a new certificate and key with the given hostnames.
	/// This won't be valid unless the client uses the fingerprint or disables verification.
	#[arg(long = "tls-generate", value_delimiter = ',')]
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub generate: Vec<String>,
}

#[derive(clap::Args, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ServerConfig {
	/// Listen for UDP packets on the given address.
	/// Defaults to `[::]:443` if not provided.
	#[arg(long)]
	pub listen: Option<net::SocketAddr>,

	#[command(flatten)]
	#[serde(default)]
	pub tls: ServerTlsConfig,
}

impl ServerConfig {
	pub fn init(self) -> anyhow::Result<Server> {
		Server::new(self)
	}
}

pub struct Server {
	quic: quinn::Endpoint,
	accept: FuturesUnordered<BoxFuture<'static, anyhow::Result<web_transport_quinn::Session>>>,
	fingerprints: Vec<String>,
}

impl Server {
	pub fn new(config: ServerConfig) -> anyhow::Result<Self> {
		// Enable BBR congestion control
		// TODO validate the implementation
		let mut transport = quinn::TransportConfig::default();
		transport.max_idle_timeout(Some(Duration::from_secs(10).try_into().unwrap()));
		transport.keep_alive_interval(Some(Duration::from_secs(4)));
		transport.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
		transport.mtu_discovery_config(None); // Disable MTU discovery
		let transport = Arc::new(transport);

		let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
		let mut serve = ServeCerts::default();

		// Load the certificate and key files based on their index.
		for cert in &config.tls.cert {
			serve.load(&cert.chain, &cert.key)?;
		}

		if !config.tls.generate.is_empty() {
			serve.generate(&config.tls.generate)?;
		}

		let fingerprints = serve.fingerprints();

		let mut tls = rustls::ServerConfig::builder_with_provider(provider)
			.with_protocol_versions(&[&rustls::version::TLS13])?
			.with_no_client_auth()
			.with_cert_resolver(Arc::new(serve));

		tls.alpn_protocols = vec![
			web_transport::quinn::ALPN.as_bytes().to_vec(),
			moq_lite::ALPN.as_bytes().to_vec(),
		];
		tls.key_log = Arc::new(rustls::KeyLogFile::new());

		let tls: quinn::crypto::rustls::QuicServerConfig = tls.try_into()?;
		let mut tls = quinn::ServerConfig::with_crypto(Arc::new(tls));
		tls.transport_config(transport.clone());

		// There's a bit more boilerplate to make a generic endpoint.
		let runtime = quinn::default_runtime().context("no async runtime")?;
		let endpoint_config = quinn::EndpointConfig::default();

		let listen = config.listen.unwrap_or("[::]:443".parse().unwrap());
		let socket = std::net::UdpSocket::bind(listen).context("failed to bind UDP socket")?;

		// Create the generic QUIC endpoint.
		let quic = quinn::Endpoint::new(endpoint_config, Some(tls), socket, runtime)
			.context("failed to create QUIC endpoint")?;

		Ok(Self {
			quic: quic.clone(),
			accept: Default::default(),
			fingerprints,
		})
	}

	pub fn fingerprints(&self) -> &[String] {
		&self.fingerprints
	}

	pub async fn accept(&mut self) -> Option<web_transport_quinn::Session> {
		loop {
			tokio::select! {
				res = self.quic.accept() => {
					let conn = res?;
					self.accept.push(Self::accept_session(conn).boxed());
				}
				Some(res) = self.accept.next() => {
					if let Ok(session) = res {
						return Some(session)
					}
				}
				_ = tokio::signal::ctrl_c() => {
					self.close();
					// Give it a chance to close.
					tokio::time::sleep(Duration::from_millis(100)).await;

					return None;
				}
			}
		}
	}

	async fn accept_session(conn: quinn::Incoming) -> anyhow::Result<web_transport_quinn::Session> {
		let mut conn = conn.accept()?;

		let handshake = conn
			.handshake_data()
			.await?
			.downcast::<quinn::crypto::rustls::HandshakeData>()
			.unwrap();

		let alpn = handshake.protocol.context("missing ALPN")?;
		let alpn = String::from_utf8(alpn).context("failed to decode ALPN")?;
		let host = handshake.server_name.unwrap_or_default();

		tracing::debug!(%host, ip = %conn.remote_address(), %alpn, "accepting");

		// Wait for the QUIC connection to be established.
		let conn = conn.await.context("failed to establish QUIC connection")?;

		let span = tracing::Span::current();
		span.record("id", conn.stable_id()); // TODO can we get this earlier?

		let session = match alpn.as_str() {
			web_transport::quinn::ALPN => {
				// Wait for the CONNECT request.
				let request = web_transport::quinn::Request::accept(conn)
					.await
					.context("failed to receive WebTransport request")?;

				// Accept the CONNECT request.
				request
					.ok()
					.await
					.context("failed to respond to WebTransport request")?
			}
			// A bit of a hack to pretend like we're a WebTransport session
			moq_lite::ALPN => {
				// Fake a URL to so we can treat it like a WebTransport session.
				let url = Url::parse(format!("moql://{}", host).as_str()).unwrap();
				web_transport::quinn::Session::raw(conn, url)
			}
			_ => anyhow::bail!("unsupported ALPN: {}", alpn),
		};

		Ok(session)
	}

	pub fn local_addr(&self) -> anyhow::Result<net::SocketAddr> {
		self.quic.local_addr().context("failed to get local address")
	}

	pub fn close(&mut self) {
		self.quic.close(quinn::VarInt::from_u32(0), b"server shutdown");
	}
}

#[derive(Debug, Default)]
struct ServeCerts {
	certs: Vec<Arc<CertifiedKey>>,
}

impl ServeCerts {
	// Load a certificate and cooresponding key from a file
	pub fn load(&mut self, chain: &PathBuf, key: &PathBuf) -> anyhow::Result<()> {
		let chain = fs::File::open(chain).context("failed to open cert file")?;
		let mut chain = io::BufReader::new(chain);

		let chain: Vec<CertificateDer> = rustls_pemfile::certs(&mut chain)
			.collect::<Result<_, _>>()
			.context("failed to read certs")?;

		anyhow::ensure!(!chain.is_empty(), "could not find certificate");

		// Read the PEM private key
		let mut keys = fs::File::open(key).context("failed to open key file")?;

		// Read the keys into a Vec so we can parse it twice.
		let mut buf = Vec::new();
		keys.read_to_end(&mut buf)?;

		let key = rustls_pemfile::private_key(&mut Cursor::new(&buf))?.context("missing private key")?;
		let key = rustls::crypto::ring::sign::any_supported_type(&key)?;

		self.certs.push(Arc::new(CertifiedKey::new(chain, key)));

		Ok(())
	}

	pub fn generate(&mut self, hostnames: &[String]) -> anyhow::Result<()> {
		let key_pair = rcgen::KeyPair::generate()?;

		let mut params = rcgen::CertificateParams::new(hostnames)?;

		// Make the certificate valid for two weeks, starting yesterday (in case of clock drift).
		// WebTransport certificates MUST be valid for two weeks at most.
		params.not_before = time::OffsetDateTime::now_utc() - time::Duration::days(1);
		params.not_after = params.not_before + time::Duration::days(14);

		// Generate the certificate
		let cert = params.self_signed(&key_pair)?;

		// Convert the rcgen type to the rustls type.
		let key = PrivatePkcs8KeyDer::from(key_pair.serialized_der());
		let key = any_supported_type(&key.into())?;

		// Create a rustls::sign::CertifiedKey
		self.certs.push(Arc::new(CertifiedKey::new(vec![cert.into()], key)));

		Ok(())
	}

	// Return the SHA256 fingerprints of all our certificates.
	pub fn fingerprints(&self) -> Vec<String> {
		self.certs
			.iter()
			.map(|ck| {
				let fingerprint = digest(&SHA256, ck.cert[0].as_ref());
				let fingerprint = hex::encode(fingerprint.as_ref());
				fingerprint
			})
			.collect()
	}

	// Return the best certificate for the given ClientHello.
	fn best_certificate(&self, client_hello: &ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
		let server_name = client_hello.server_name()?;
		let dns_name = webpki::DnsNameRef::try_from_ascii_str(server_name).ok()?;

		for ck in &self.certs {
			// TODO I gave up on caching the parsed result because of lifetime hell.
			// I think some unsafe is needed?
			let leaf = ck.end_entity_cert().expect("missing certificate");
			let parsed = webpki::EndEntityCert::try_from(leaf.as_ref()).expect("failed to parse certificate");

			if parsed.verify_is_valid_for_dns_name(dns_name).is_ok() {
				return Some(ck.clone());
			}
		}

		None
	}
}

impl ResolvesServerCert for ServeCerts {
	fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
		if let Some(cert) = self.best_certificate(&client_hello) {
			return Some(cert);
		}

		// If this happens, it means the client was trying to connect to an unknown hostname.
		// We do our best and return the first certificate.
		tracing::warn!(server_name = ?client_hello.server_name(), "no SNI certificate found");

		self.certs.first().cloned()
	}
}
