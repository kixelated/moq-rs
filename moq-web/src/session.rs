use moq_karp::moq_transfork::{self, web_transport};
use url::Url;

use crate::{Error, Result};

pub async fn connect(addr: &Url) -> Result<moq_transfork::Session> {
	tracing::info!("connecting to: {}", addr);

	let client = web_transport::Client::new().congestion_control(web_transport::CongestionControl::LowLatency);

	let session = match addr.scheme() {
		"http" => {
			// TODO Unfortunately, WebTransport doesn't work correctly with self-signed certificates.
			// Until that gets fixed, we need to perform a HTTP request to fetch the certificate hashes.
			let fingerprint = fingerprint(addr).await?;
			let client = client.server_certificate_hashes(vec![fingerprint]);
			// Make a copy of the address, changing it from HTTP to HTTPS for WebTransport:
			let mut addr = addr.clone();
			let _ = addr.set_scheme("https");
			client.connect(&addr).await?
		}
		"https" => client.connect(addr).await?,
		_ => return Err(Error::InvalidUrl),
	};

	let session = moq_transfork::Session::connect(session).await?;

	Ok(session)
}

async fn fingerprint(url: &Url) -> Result<Vec<u8>> {
	let mut fingerprint = url.clone();
	fingerprint.set_path("fingerprint");

	let resp = gloo_net::http::Request::get(fingerprint.as_str()).send().await?;

	let body = resp.text().await?;
	let fingerprint = hex::decode(body.trim()).map_err(|_| Error::InvalidFingerprint)?;

	Ok(fingerprint)
}
