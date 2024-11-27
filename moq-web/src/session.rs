use moq_karp::moq_transfork::{self, web_transport};
use url::Url;

use crate::{Error, Result};

pub async fn connect(addr: &Url) -> Result<moq_transfork::Session> {
    tracing::info!("connecting to: {}", addr);

    if addr.scheme() != "https" {
        return Err(Error::InvalidUrl);
    }

    let session = web_transport::wasm::Session::build(addr.clone())
        .allow_pooling(false)
        .congestion_control(web_transport::wasm::CongestionControl::LowLatency)
        .require_unreliable(true);

    // TODO Unfortunately, WebTransport doesn't work correctly with self-signed certificates.
    // Until that gets fixed, we need to perform a HTTP request to fetch the certificate hashes.
    let session = match addr.host_str() {
        Some("localhost") => {
            let fingerprint = fingerprint(addr).await?;
            session.server_certificate_hashes(vec![fingerprint])
        }
        _ => session,
    };

    let session = session.connect().await?;
    let session = moq_transfork::Session::connect(session.into()).await?;

    Ok(session)
}

async fn fingerprint(url: &Url) -> Result<Vec<u8>> {
    let mut fingerprint = url.clone();
    fingerprint.set_path("fingerprint");

    let resp = gloo_net::http::Request::get(fingerprint.as_str())
        .send()
        .await?;

    let body = resp.text().await?;
    let fingerprint = hex::decode(body.trim()).map_err(|_| Error::InvalidFingerprint)?;

    Ok(fingerprint)
}
