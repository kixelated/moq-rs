use std::{net, sync::Arc};

use axum::{extract::State, http::Method, response::IntoResponse, routing::get, Router};
use axum_server::tls_rustls::RustlsAcceptor;
use tower_http::cors::{Any, CorsLayer};

pub struct WebConfig {
	pub bind: net::SocketAddr,
	pub tls: moq_native::tls::Config,
}

// Run a HTTP server using Axum
// TODO remove this when Chrome adds support for self-signed certificates using WebTransport
pub struct Web {
	app: Router,
	server: axum_server::Server<RustlsAcceptor>,
}

impl Web {
	pub fn new(config: WebConfig) -> Self {
		// Get the first certificate's fingerprint.
		// TODO serve all of them so we can support multiple signature algorithms.
		let fingerprint = config.tls.fingerprints.first().expect("missing certificate").clone();

		let mut tls = config.tls.server.expect("missing server configuration");
		tls.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
		let tls = axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(tls));

		let app = Router::new()
			.route("/fingerprint", get(serve_fingerprint))
			.layer(CorsLayer::new().allow_origin(Any).allow_methods([Method::GET]))
			.with_state(fingerprint);

		let server = axum_server::bind_rustls(config.bind, tls);

		Self { app, server }
	}

	pub async fn run(self) -> anyhow::Result<()> {
		self.server.serve(self.app.into_make_service()).await?;
		Ok(())
	}
}

async fn serve_fingerprint(State(fingerprint): State<String>) -> impl IntoResponse {
	fingerprint
}
