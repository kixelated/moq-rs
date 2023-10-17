use anyhow::Context;
use ring::digest::{digest, SHA256};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use rustls::{Certificate, PrivateKey, RootCertStore};
use rustls::{ClientConfig, ServerConfig};
use std::fs;
use std::io::{self, Cursor, Read};
use std::path;
use std::sync::Arc;
use webpki::{DnsNameRef, EndEntityCert};

use crate::Config;

#[derive(Clone)]
pub struct Tls {
	// Support serving multiple certificates, choosing one that looks valid for the given SNI.
	// We store the parsed certificate, and the certified cert/key that rustls expects
	serve: Arc<ServeCerts>,

	// Accept any cert that is trusted by the system's native trust store.
	accept: Arc<RootCertStore>,
}

impl Tls {
	pub fn load(config: &Config) -> anyhow::Result<Self> {
		let mut serve = ServeCerts::default();

		// Load the certificate and key files based on their index.
		anyhow::ensure!(
			config.tls_cert.len() == config.tls_key.len(),
			"--tls-cert and --tls-key counts differ"
		);
		for (chain, key) in config.tls_cert.iter().zip(config.tls_key.iter()) {
			serve.load(chain, key)?;
		}

		// Create a list of acceptable root certificates.
		let mut roots = RootCertStore::empty();

		// Add the platform's native root certificates.
		for cert in rustls_native_certs::load_native_certs().context("could not load platform certs")? {
			roots.add(&Certificate(cert.0)).context("failed to add root cert")?;
		}

		let certs = Self {
			serve: Arc::new(serve),
			accept: Arc::new(roots),
		};

		Ok(certs)
	}

	pub fn client(&self) -> ClientConfig {
		rustls::ClientConfig::builder()
			.with_safe_defaults()
			.with_root_certificates(self.accept.clone())
			.with_no_client_auth()
	}

	pub fn server(&self) -> ServerConfig {
		rustls::ServerConfig::builder()
			.with_safe_defaults()
			.with_no_client_auth()
			.with_cert_resolver(self.serve.clone())
	}

	// Return the SHA256 fingerprint of our certificates.
	pub fn fingerprints(&self) -> Vec<String> {
		self.serve.fingerprints()
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
