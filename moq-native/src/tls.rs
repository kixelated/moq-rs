use anyhow::Context;
use clap::Parser;
use ring::digest::{digest, SHA256};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use rustls::RootCertStore;
use std::fs;
use std::io::{self, Cursor, Read};
use std::path;
use std::sync::Arc;

#[derive(Parser, Clone, Default)]
#[group(id = "tls")]
pub struct Args {
	/// Use the certificates at this path, encoded as PEM.
	///
	/// You can use this option multiple times for multiple certificates.
	/// The first match for the provided SNI will be used, otherwise the last cert will be used.
	/// You also need to provide the private key multiple times via `key``.
	#[arg(long = "tls-cert")]
	pub cert: Vec<path::PathBuf>,

	/// Use the private key at this path, encoded as PEM.
	///
	/// There must be a key for every certificate provided via `cert`.
	#[arg(long = "tls-key")]
	pub key: Vec<path::PathBuf>,

	/// Use the TLS root at this path, encoded as PEM.
	///
	/// This value can be provided multiple times for multiple roots.
	/// If this is empty, system roots will be used instead
	#[arg(long = "tls-root")]
	pub root: Vec<path::PathBuf>,

	/// Danger: Disable TLS certificate verification.
	///
	/// Fine for local development and between relays, but should be used in caution in production.
	#[arg(long = "tls-disable-verify")]
	pub disable_verify: bool,
}

#[derive(Clone)]
pub struct Config {
	pub client: rustls::ClientConfig,
	pub server: Option<rustls::ServerConfig>,
	pub fingerprints: Vec<String>,
}

impl Args {
	pub fn load(&self) -> anyhow::Result<Config> {
		let provider = Arc::new(rustls::crypto::ring::default_provider());
		let mut serve = ServeCerts::default();

		// Load the certificate and key files based on their index.
		anyhow::ensure!(
			self.cert.len() == self.key.len(),
			"--tls-cert and --tls-key counts differ"
		);
		for (chain, key) in self.cert.iter().zip(self.key.iter()) {
			serve.load(chain, key)?;
		}

		// Create a list of acceptable root certificates.
		let mut roots = RootCertStore::empty();

		if self.root.is_empty() {
			// Add the platform's native root certificates.
			for cert in rustls_native_certs::load_native_certs().context("could not load platform certs")? {
				roots.add(cert).context("failed to add root cert")?;
			}
		} else {
			// Add the specified root certificates.
			for root in &self.root {
				let root = fs::File::open(root).context("failed to open root cert file")?;
				let mut root = io::BufReader::new(root);

				let root = rustls_pemfile::certs(&mut root)
					.next()
					.context("no roots found")?
					.context("failed to read root cert")?;

				roots.add(root).context("failed to add root cert")?;
			}
		}

		// Create the TLS configuration we'll use as a client (relay -> relay)
		let mut client = rustls::ClientConfig::builder_with_provider(provider.clone())
			.with_protocol_versions(&[&rustls::version::TLS13])?
			.with_root_certificates(roots)
			.with_no_client_auth();

		// Allow disabling TLS verification altogether.
		if self.disable_verify {
			let noop = NoCertificateVerification(provider.clone());
			client.dangerous().set_certificate_verifier(Arc::new(noop));
		}

		let fingerprints = serve.fingerprints();

		// Create the TLS configuration we'll use as a server (relay <- browser)
		let server = if !self.key.is_empty() {
			Some(
				rustls::ServerConfig::builder_with_provider(provider)
					.with_protocol_versions(&[&rustls::version::TLS13])?
					.with_no_client_auth()
					.with_cert_resolver(Arc::new(serve)),
			)
		} else {
			None
		};

		Ok(Config {
			server,
			client,
			fingerprints,
		})
	}
}

#[derive(Default, Debug)]
struct ServeCerts {
	list: Vec<Arc<CertifiedKey>>,
}

impl ServeCerts {
	// Load a certificate and cooresponding key from a file
	pub fn load(&mut self, chain: &path::PathBuf, key: &path::PathBuf) -> anyhow::Result<()> {
		// Read the PEM certificate chain
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

		let certified = Arc::new(CertifiedKey::new(chain, key));
		self.list.push(certified);

		Ok(())
	}

	// Return the SHA256 fingerprint of our certificates.
	pub fn fingerprints(&self) -> Vec<String> {
		self.list
			.iter()
			.map(|ck| {
				let fingerprint = digest(&SHA256, ck.cert[0].as_ref());
				let fingerprint = hex::encode(fingerprint.as_ref());
				fingerprint
			})
			.collect()
	}
}

impl ResolvesServerCert for ServeCerts {
	fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
		if let Some(name) = client_hello.server_name() {
			if let Ok(dns_name) = webpki::DnsNameRef::try_from_ascii_str(name) {
				for ck in &self.list {
					// TODO I gave up on caching the parsed result because of lifetime hell.
					// If this shows up on benchmarks, somebody should fix it.
					let leaf = ck.end_entity_cert().expect("missing certificate");
					let parsed = webpki::EndEntityCert::try_from(leaf.as_ref()).expect("failed to parse certificate");

					if parsed.verify_is_valid_for_dns_name(dns_name).is_ok() {
						return Some(ck.clone());
					}
				}
			}
		}

		// Default to the last certificate if we couldn't find one.
		self.list.last().cloned()
	}
}

#[derive(Debug)]
pub struct NoCertificateVerification(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for NoCertificateVerification {
	fn verify_server_cert(
		&self,
		_end_entity: &CertificateDer<'_>,
		_intermediates: &[CertificateDer<'_>],
		_server_name: &ServerName<'_>,
		_ocsp: &[u8],
		_now: UnixTime,
	) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
		Ok(rustls::client::danger::ServerCertVerified::assertion())
	}

	fn verify_tls12_signature(
		&self,
		message: &[u8],
		cert: &CertificateDer<'_>,
		dss: &rustls::DigitallySignedStruct,
	) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
		rustls::crypto::verify_tls12_signature(message, cert, dss, &self.0.signature_verification_algorithms)
	}

	fn verify_tls13_signature(
		&self,
		message: &[u8],
		cert: &CertificateDer<'_>,
		dss: &rustls::DigitallySignedStruct,
	) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
		rustls::crypto::verify_tls13_signature(message, cert, dss, &self.0.signature_verification_algorithms)
	}

	fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
		self.0.signature_verification_algorithms.supported_schemes()
	}
}
