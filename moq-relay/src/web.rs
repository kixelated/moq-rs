use std::net;

use axum::{extract::State, http::Method, response::IntoResponse, routing::get, Router};
use hyper_serve::accept::DefaultAcceptor;
use tower_http::cors::{Any, CorsLayer};

pub struct WebConfig {
	pub bind: net::SocketAddr,
	pub tls: moq_native::tls::Config,
}

// Run a HTTP server using Axum
// TODO remove this when Chrome adds support for self-signed certificates using WebTransport
pub struct Web {
	app: Router,
	server: hyper_serve::Server<DefaultAcceptor>,
}

impl Web {
	pub fn new(config: WebConfig) -> Self {
		// Get the first certificate's fingerprint.
		// TODO serve all of them so we can support multiple signature algorithms.
		let fingerprint = config.tls.fingerprints.first().expect("missing certificate").clone();

		let app = Router::new()
			.route("/fingerprint", get(serve_fingerprint))
			.layer(CorsLayer::new().allow_origin(Any).allow_methods([Method::GET]))
			.with_state(fingerprint);

		let server = hyper_serve::bind(config.bind);

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
