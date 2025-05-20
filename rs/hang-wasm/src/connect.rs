use std::{cell::RefCell, collections::HashMap, sync::Arc};

use hang::moq_lite::{self, web_transport};
use tokio::sync::watch;
use url::Url;
use wasm_bindgen_futures::spawn_local;

type ConnectionPending = watch::Receiver<Option<Result<moq_lite::Session, ConnectError>>>;

// Can't use LazyLock in WASM because nothing is Sync
thread_local! {
	static POOL: RefCell<HashMap<Url, ConnectionPending>> = RefCell::new(HashMap::new());
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum ConnectError {
	#[error("invalid url: {0}")]
	InvalidUrl(#[from] url::ParseError),

	#[error("invalid scheme")]
	InvalidScheme,

	#[error("moq error: {0}")]
	Moq(#[from] moq_lite::Error),

	#[error("webtransport error: {0}")]
	WebTransport(#[from] web_transport::Error),

	#[error("invalid fingerprint")]
	InvalidFingerprint,

	#[error("http error: {0}")]
	Http(Arc<gloo_net::Error>),
}

impl From<gloo_net::Error> for ConnectError {
	fn from(err: gloo_net::Error) -> Self {
		Self::Http(Arc::new(err))
	}
}

#[derive(Clone)]
pub struct Connect {
	pending: ConnectionPending,
	pub path: String,
}

impl Connect {
	pub fn new(addr: Url) -> Self {
		let path = addr.path().to_string();

		// Use a global cache to share sessions between elements.
		let pending = POOL.with(|cache| {
			let mut cache = cache.borrow_mut();

			let entry = cache.entry(addr.clone()).or_insert_with(|| Self::create(addr));
			entry.clone()
		});

		Self { path, pending }
	}

	fn create(addr: Url) -> ConnectionPending {
		let (tx, rx) = watch::channel(None);

		// Use a background task to make `connect` cancel safe.
		spawn_local(async move {
			let session = Self::run(&addr).await;
			tx.send(Some(session.clone())).ok();

			if let Ok(session) = session {
				tokio::select! {
					// Close the session gracefully when there are no more consumers.
					_ = tx.closed() => session.close(moq_lite::Error::Cancel),

					// Remove the session from the cache when it's closed.
					err = session.closed() => {
						tracing::warn!(?err, "session closed");
						POOL.with(|cache| {
							cache.borrow_mut().remove(&addr);
						});
					},
				}
			}
		});

		rx
	}

	async fn run(addr: &Url) -> Result<moq_lite::Session, ConnectError> {
		let client =
			web_transport::ClientBuilder::new().with_congestion_control(web_transport::CongestionControl::LowLatency);

		let session = match addr.scheme() {
			"http" => {
				// TODO Unfortunately, WebTransport doesn't work correctly with self-signed certificates.
				// Until that gets fixed, we need to perform a HTTP request to fetch the certificate hashes.
				let fingerprint = Self::fingerprint(addr).await?;
				let client = client.with_server_certificate_hashes(vec![fingerprint])?;

				// Make a copy of the address, changing it from HTTP to HTTPS for WebTransport:
				let mut addr = addr.clone();
				let _ = addr.set_scheme("https");
				client.connect(&addr).await?
			}
			"https" => {
				let client = client.with_system_roots()?;
				client.connect(addr).await?
			}
			_ => return Err(ConnectError::InvalidScheme),
		};

		let session = moq_lite::Session::connect(session).await?;
		Ok(session)
	}

	async fn fingerprint(url: &Url) -> Result<Vec<u8>, ConnectError> {
		let mut fingerprint = url.clone();
		fingerprint.set_path("certificate.sha256");

		let resp = gloo_net::http::Request::get(fingerprint.as_str()).send().await?;

		let body = resp.text().await?;
		let fingerprint = hex::decode(body.trim()).map_err(|_| ConnectError::InvalidFingerprint)?;

		Ok(fingerprint)
	}

	pub async fn established(&mut self) -> Result<moq_lite::Session, ConnectError> {
		self.pending
			.wait_for(Option::is_some)
			.await
			.expect("background task panicked")
			.as_ref()
			.unwrap()
			.clone()
	}
}
