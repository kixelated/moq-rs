use axum::{http::Method, routing::get, Router};
use hyper_serve::accept::DefaultAcceptor;
use std::net;
use tower_http::cors::{Any, CorsLayer};

// Run a HTTP server using Axum
// TODO remove this when Chrome adds support for self-signed certificates using WebTransport
pub struct FingerprintServer {
	app: Router,
	server: hyper_serve::Server<DefaultAcceptor>,
}

impl FingerprintServer {
	pub fn new(bind: net::SocketAddr, tls: moq_native::tls::Config) -> Self {
		// Get the first certificate's fingerprint.
		// TODO serve all of them so we can support multiple signature algorithms.
		let fingerprint = tls.fingerprints.first().expect("missing certificate").clone();

		let app = Router::new()
			.route("/fingerprint", get(fingerprint))
			.layer(CorsLayer::new().allow_origin(Any).allow_methods([Method::GET]));

		let server = hyper_serve::bind(bind);

		Self { app, server }
	}

	pub async fn run(self) -> anyhow::Result<()> {
		self.server.serve(self.app.into_make_service()).await?;
		Ok(())
	}
}
