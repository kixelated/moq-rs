use anyhow::{self, Context};
use env_logger;
use log;
use std::io::{self, Cursor, Read};
use tokio;
use webtransport_quinn;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();
	// let atom = read_atom(io::stdin().by_ref());
	//dbg!(&atom);
	let ftyp = read_atom(io::stdin().by_ref())?;
	anyhow::ensure!(&ftyp[4..8] == b"ftyp", "expected ftyp atom");

	let moov = read_atom(io::stdin().by_ref())?;
	anyhow::ensure!(&moov[4..8] == b"moov", "expected moov atom");

	dbg!(&ftyp, &moov);
	let mut ftyp_reader = Cursor::new(ftyp);
	let parsed_ftyp = mp4::BoxHeader::read(&mut ftyp_reader);
	let mut moov_reader = Cursor::new(moov);
	let parsed_moov = mp4::BoxHeader::read(&mut moov_reader);
	dbg!(&parsed_ftyp, &parsed_moov);

	let mut tls_config = rustls::ClientConfig::builder()
		.with_safe_defaults()
		.with_custom_certificate_verifier(SkipServerVerification::new()) // TODO: Replace this!!!
		.with_no_client_auth();

	tls_config.alpn_protocols = vec![webtransport_quinn::ALPN.to_vec()]; // this one is important

	let config = quinn::ClientConfig::new(std::sync::Arc::new(tls_config));

	let addr = "[::]:0".parse()?;
	let mut client = quinn::Endpoint::client(addr)?;
	client.set_default_client_config(config);

	let uri = "https://localhost:4443/webtransport/devious-baton"
		.try_into()
		.context("failed to parse uri")?;

	// Use a helper method to convert URI to host/port.
	let conn = webtransport_quinn::dial(&client, &uri).await?;
	log::info!("connecting to {}", uri);

	let conn = conn.await?;
	log::info!("established QUIC connection");

	// Perform the WebTransport handshake.
	let session = webtransport_quinn::connect(conn, &uri).await?;
	log::info!("established WebTransport session");

	// TODO MoQT stuffs

	Ok(())
}

// Read a full MP4 atom into a vector.
fn read_atom<R: Read>(reader: &mut R) -> anyhow::Result<Vec<u8>> {
	// Read the 8 bytes for the size + type
	let mut buf = [0u8; 8];
	reader.read_exact(&mut buf)?;

	// Convert the first 4 bytes into the size.
	let size = u32::from_be_bytes(buf[0..4].try_into()?) as u64;
	//let typ = &buf[4..8].try_into().ok().unwrap();

	let mut raw = buf.to_vec();

	let mut limit = match size {
		// Runs until the end of the file.
		0 => reader.take(u64::MAX),

		// The next 8 bytes are the extended size to be used instead.
		1 => {
			reader.read_exact(&mut buf)?;
			let size_large = u64::from_be_bytes(buf);
			anyhow::ensure!(size_large >= 16, "impossible extended box size: {}", size_large);

			reader.take(size_large - 16)
		}

		2..=7 => {
			anyhow::bail!("impossible box size: {}", size)
		}

		// Otherwise read based on the size.
		size => reader.take(size - 8),
	};

	// Append to the vector and return it.
	limit.read_to_end(&mut raw)?;

	Ok(raw)
}

// Implementation of `ServerCertVerifier` that verifies everything as trustworthy.
// WARNING: Don't use this in production.
struct SkipServerVerification;

impl SkipServerVerification {
	fn new() -> std::sync::Arc<Self> {
		std::sync::Arc::new(Self)
	}
}

impl rustls::client::ServerCertVerifier for SkipServerVerification {
	fn verify_server_cert(
		&self,
		_end_entity: &rustls::Certificate,
		_intermediates: &[rustls::Certificate],
		_server_name: &rustls::ServerName,
		_scts: &mut dyn Iterator<Item = &[u8]>,
		_ocsp_response: &[u8],
		_now: std::time::SystemTime,
	) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
		Ok(rustls::client::ServerCertVerified::assertion())
	}
}
