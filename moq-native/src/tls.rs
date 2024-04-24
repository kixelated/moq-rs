use anyhow::Context;
use clap::Parser;
use ring::digest::{digest, SHA256};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use rustls::{Certificate, PrivateKey, RootCertStore};
use std::io::{self, Cursor, Read};
use std::path;
use std::sync::Arc;
use std::{fs, time};
use webpki::{DnsNameRef, EndEntityCert};

#[derive(Parser, Clone)]
#[group(id = "tls")]
pub struct Cli {
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

impl Cli {
	pub fn load(&self) -> anyhow::Result<Config> {
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
				roots.add(&Certificate(cert.0)).context("failed to add root cert")?;
			}
		} else {
			// Add the specified root certificates.
			for root in &self.root {
				let root = fs::File::open(root).context("failed to open root cert file")?;
				let mut root = io::BufReader::new(root);
				let root = rustls_pemfile::certs(&mut root).context("failed to read root cert")?;
				anyhow::ensure!(root.len() == 1, "expected a single root cert");
				let root = Certificate(root[0].to_owned());

				roots.add(&root).context("failed to add root cert")?;
			}
		}

		// Create the TLS configuration we'll use as a client (relay -> relay)
		let mut client = rustls::ClientConfig::builder()
			.with_safe_defaults()
			.with_root_certificates(roots)
			.with_no_client_auth();

		// Allow disabling TLS verification altogether.
		if self.disable_verify {
			let noop = NoCertificateVerification {};
			client.dangerous().set_certificate_verifier(Arc::new(noop));
		}

		let fingerprints = serve.fingerprints();

		// Create the TLS configuration we'll use as a server (relay <- browser)
		let server = if !self.key.is_empty() {
			Some(
				rustls::ServerConfig::builder()
					.with_safe_defaults()
					.with_no_client_auth()
					.with_cert_resolver(Arc::new(serve)),
			)
		} else {
			None
		};

		let certs = Config {
			server,
			client,
			fingerprints,
		};

		Ok(certs)
	}
}

#[derive(Default)]
struct ServeCerts {
	list: Vec<Arc<CertifiedKey>>,
}

impl ServeCerts {
	// Load a certificate and cooresponding key from a file
	pub fn load(&mut self, chain: &path::PathBuf, key: &path::PathBuf) -> anyhow::Result<()> {
		// Read the PEM certificate chain
		let chain = fs::File::open(chain).context("failed to open cert file")?;
		let mut chain = io::BufReader::new(chain);

		let chain: Vec<Certificate> = rustls_pemfile::certs(&mut chain)?
			.into_iter()
			.map(Certificate)
			.collect();

		anyhow::ensure!(!chain.is_empty(), "could not find certificate");

		// Read the PEM private key
		let mut keys = fs::File::open(key).context("failed to open key file")?;

		// Read the keys into a Vec so we can parse it twice.
		let mut buf = Vec::new();
		keys.read_to_end(&mut buf)?;

		// Try to parse a PKCS#8 key
		// -----BEGIN PRIVATE KEY-----
		let mut keys = rustls_pemfile::pkcs8_private_keys(&mut Cursor::new(&buf))?;

		// Try again but with EC keys this time
		// -----BEGIN EC PRIVATE KEY-----
		if keys.is_empty() {
			keys = rustls_pemfile::ec_private_keys(&mut Cursor::new(&buf))?
		};

		anyhow::ensure!(!keys.is_empty(), "could not find private key");
		anyhow::ensure!(keys.len() < 2, "expected a single key");

		let key = PrivateKey(keys.remove(0));
		let key = rustls::sign::any_supported_type(&key)?;

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
			if let Ok(dns_name) = DnsNameRef::try_from_ascii_str(name) {
				for ck in &self.list {
					// TODO I gave up on caching the parsed result because of lifetime hell.
					// If this shows up on benchmarks, somebody should fix it.
					let leaf = ck.cert.first().expect("missing certificate");
					let parsed = EndEntityCert::try_from(leaf.0.as_ref()).expect("failed to parse certificate");

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

pub struct NoCertificateVerification {}

impl rustls::client::ServerCertVerifier for NoCertificateVerification {
	fn verify_server_cert(
		&self,
		_end_entity: &rustls::Certificate,
		_intermediates: &[rustls::Certificate],
		_server_name: &rustls::ServerName,
		_scts: &mut dyn Iterator<Item = &[u8]>,
		_ocsp_response: &[u8],
		_now: time::SystemTime,
	) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
		Ok(rustls::client::ServerCertVerified::assertion())
	}
}
