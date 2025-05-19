use super::Config;

use anyhow::Context;
use axum::handler::HandlerWithoutStateExt;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{http::Method, routing::get, Router};
use clap::Args;
use hang::{cmaf, moq_lite};
use hang::{BroadcastConsumer, BroadcastProducer};
use moq_lite::web_transport;
use moq_native::quic;
use std::path::PathBuf;
use tokio::io::AsyncRead;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

/// Host a server, accepting connections from clients.
#[derive(Args, Clone)]
pub struct ServerConfig {
	/// Optionally serve any HTML files in the given directory.
	#[arg(long)]
	dir: Option<PathBuf>,
}

pub struct Server {
	config: Config,
	server_config: ServerConfig,
	tls: moq_native::tls::Config,
}

impl Server {
	pub async fn new(mut config: Config, server_config: ServerConfig) -> anyhow::Result<Self> {
		config.bind = tokio::net::lookup_host(config.bind)
			.await
			.context("invalid bind address")?
			.next()
			.context("invalid bind address")?;

		let tls = config.tls.load()?;
		if tls.server.is_none() {
			anyhow::bail!("missing TLS certificates");
		}

		Ok(Self {
			config,
			server_config,
			tls,
		})
	}

	pub async fn run<T: AsyncRead + Unpin>(self, input: &mut T) -> anyhow::Result<()> {
		let broadcast = hang::Broadcast {
			room: self.config.room.clone(),
			name: self.config.name.clone(),
		};

		let producer = BroadcastProducer::new(broadcast);
		let consumer = producer.consume();

		tokio::select! {
			res = self.accept(consumer) => res,
			res = self.publish(producer, input) => res,
			res = self.web() => res,
		}
	}

	async fn accept(&self, consumer: BroadcastConsumer) -> anyhow::Result<()> {
		let quic = quic::Endpoint::new(quic::Config {
			bind: self.config.bind,
			tls: self.tls.clone(),
		})?;

		let mut quic = quic.server.context("missing TLS certificate")?;
		tracing::info!(addr = %self.config.bind, "listening");

		let mut conn_id = 0;

		while let Some(session) = quic.accept().await {
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

				session.publish(consumer.inner);
			});
		}

		Ok(())
	}

	async fn publish<T: AsyncRead + Unpin>(&self, producer: BroadcastProducer, input: &mut T) -> anyhow::Result<()> {
		let mut import = cmaf::Import::new(producer);

		import
			.init_from(input)
			.await
			.context("failed to initialize cmaf from input")?;

		tracing::info!("initialized");
		tracing::info!(room = %self.config.room, name = %self.config.name, "publishing");

		import.read_from(input).await?;

		Ok(())
	}

	// Run a HTTP server using Axum to serve the certificate fingerprint.
	async fn web(&self) -> anyhow::Result<()> {
		// Get the first certificate's fingerprint.
		// TODO serve all of them so we can support multiple signature algorithms.
		let fingerprint = self.tls.fingerprints.first().expect("missing certificate").clone();

		async fn handle_404() -> impl IntoResponse {
			(StatusCode::NOT_FOUND, "Not found")
		}

		let mut app = Router::new()
			.route("/certificate.sha256", get(fingerprint))
			.layer(CorsLayer::new().allow_origin(Any).allow_methods([Method::GET]));

		// If a public directory is provided, serve it.
		// We use this for local development to serve the index.html file and friends.
		if let Some(public) = self.server_config.dir.as_ref() {
			tracing::info!(?public, "serving directory");

			let public = ServeDir::new(public).not_found_service(handle_404.into_service());
			app = app.fallback_service(public);
		} else {
			app = app.fallback_service(handle_404.into_service());
		}

		let server = hyper_serve::bind(self.config.bind);
		server.serve(app.into_make_service()).await?;

		Ok(())
	}
}
