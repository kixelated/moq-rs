use anyhow::Context;
use axum::handler::HandlerWithoutStateExt;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{http::Method, routing::get, Router};
use hang::{cmaf, moq_lite};
use hang::{BroadcastConsumer, BroadcastProducer};
use moq_lite::web_transport;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::io::AsyncRead;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

pub async fn server<T: AsyncRead + Unpin>(
	config: moq_native::ServerConfig,
	public: Option<PathBuf>,
	input: &mut T,
) -> anyhow::Result<()> {
	let mut listen = config.listen.unwrap_or("[::]:443".parse().unwrap());
	listen = tokio::net::lookup_host(listen)
		.await
		.context("invalid listen address")?
		.next()
		.context("invalid listen address")?;

	let server = config.init()?;
	let fingerprints = server.fingerprints().to_vec();

	let producer = BroadcastProducer::new();
	let consumer = producer.consume();

	tokio::select! {
		res = accept(server, consumer) => res,
		res = publish(producer, input) => res,
		res = web(listen, fingerprints, public) => res,
	}
}

async fn accept(mut server: moq_native::Server, consumer: BroadcastConsumer) -> anyhow::Result<()> {
	let mut conn_id = 0;

	tracing::info!(addr = ?server.local_addr(), "listening");

	while let Some(session) = server.accept().await {
		let id = conn_id;
		conn_id += 1;

		let consumer = consumer.clone();

		// Handle the connection in a new task.
		tokio::spawn(async move {
			let session: web_transport::Session = session.into();
			let mut session = moq_lite::Session::accept(session)
				.await
				.expect("failed to accept session");

			tracing::info!(?id, "accepted session");

			// The path is relative to the URL, so it's empty because we only publish one broadcast.
			session.publish("", consumer.inner.clone());
		});
	}

	Ok(())
}

async fn publish<T: AsyncRead + Unpin>(producer: BroadcastProducer, input: &mut T) -> anyhow::Result<()> {
	let mut import = cmaf::Import::new(producer);

	import
		.init_from(input)
		.await
		.context("failed to initialize cmaf from input")?;

	tracing::info!("initialized");

	import.read_from(input).await?;

	Ok(())
}

// Run a HTTP server using Axum to serve the certificate fingerprint.
async fn web(bind: SocketAddr, fingerprints: Vec<String>, public: Option<PathBuf>) -> anyhow::Result<()> {
	// Get the first certificate's fingerprint.
	// TODO serve all of them so we can support multiple signature algorithms.
	let fingerprint = fingerprints.first().expect("missing certificate").clone();

	async fn handle_404() -> impl IntoResponse {
		(StatusCode::NOT_FOUND, "Not found")
	}

	let mut app = Router::new()
		.route("/certificate.sha256", get(fingerprint))
		.layer(CorsLayer::new().allow_origin(Any).allow_methods([Method::GET]));

	// If a public directory is provided, serve it.
	// We use this for local development to serve the index.html file and friends.
	if let Some(public) = public.as_ref() {
		tracing::info!(?public, "serving directory");

		let public = ServeDir::new(public).not_found_service(handle_404.into_service());
		app = app.fallback_service(public);
	} else {
		app = app.fallback_service(handle_404.into_service());
	}

	let server = hyper_serve::bind(bind);
	server.serve(app.into_make_service()).await?;

	Ok(())
}
