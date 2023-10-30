use std::sync::Arc;

use axum::{extract::State, http::Method, response::IntoResponse, routing::get, Router};
use axum_server::{tls_rustls::RustlsAcceptor, Server};
use tower_http::cors::{Any, CorsLayer};

use crate::{Config, Tls};

// Run a HTTP server using Axum
// TODO remove this when Chrome adds support for self-signed certificates using WebTransport
pub struct Web {
	app: Router,
	server: Server<RustlsAcceptor>,
}

impl Web {
	pub fn new(config: Config, tls: Tls) -> Self {
		// Get the first certificate's fingerprint.
		// TODO serve all of them so we can support multiple signature algorithms.
		let fingerprint = tls.fingerprints.first().expect("missing certificate").clone();

		let mut tls_config = tls.server.clone();
		tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
		let tls_config = axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(tls_config));

		let app = Router::new()
			.route("/fingerprint", get(serve_fingerprint))
			.layer(CorsLayer::new().allow_origin(Any).allow_methods([Method::GET]))
			.with_state(fingerprint);

		let server = axum_server::bind_rustls(config.listen, tls_config);

		Self { app, server }
	}

	pub async fn serve(self) -> anyhow::Result<()> {
		self.server.serve(self.app.into_make_service()).await?;
		Ok(())
	}
}

async fn serve_fingerprint(State(fingerprint): State<String>) -> impl IntoResponse {
	fingerprint
}
