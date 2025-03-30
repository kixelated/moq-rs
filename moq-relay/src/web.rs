use std::{
	net,
	pin::Pin,
	task::{ready, Context, Poll},
};

use axum::{
	body::Body,
	extract::Path,
	http::{Method, StatusCode},
	response::{IntoResponse, Response},
	routing::get,
	Router,
};
use bytes::Bytes;
use futures::FutureExt;
use hyper_serve::accept::DefaultAcceptor;
use moq_transfork::Filter;
use std::future::Future;
use tower_http::cors::{Any, CorsLayer};

use crate::Cluster;

pub struct WebConfig {
	pub bind: net::SocketAddr,
	pub tls: moq_native::tls::Config,
	pub cluster: Cluster,
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
			.route("/fingerprint", get(fingerprint))
			.route(
				"/announced",
				get({
					let cluster = config.cluster.clone();
					move || serve_announced(Path("".to_string()), cluster.clone())
				}),
			)
			.route(
				"/announced/{*prefix}",
				get({
					let cluster = config.cluster.clone();
					move |path| serve_announced(path, cluster)
				}),
			)
			.route(
				"/fetch/{*path}",
				get({
					let cluster = config.cluster.clone();
					move |path| serve_fetch(path, cluster)
				}),
			)
			.layer(CorsLayer::new().allow_origin(Any).allow_methods([Method::GET]));

		let server = hyper_serve::bind(config.bind);

		Self { app, server }
	}

	pub async fn run(self) -> anyhow::Result<()> {
		self.server.serve(self.app.into_make_service()).await?;
		Ok(())
	}
}

/// Serve the announced tracks for a given prefix.
async fn serve_announced(Path(path): Path<String>, cluster: Cluster) -> impl IntoResponse {
	// Make anything without a / prefix private.
	let filter = if path.is_empty() {
		Filter::Any
	} else if let Some((prefix, suffix)) = path.split_once("*") {
		Filter::Wildcard {
			prefix: prefix.to_string(),
			suffix: suffix.to_string(),
		}
	} else {
		Filter::Prefix(path)
	};

	let mut local = cluster.locals.announced(filter.clone());
	let mut remote = cluster.remotes.announced(filter);

	let mut tracks = Vec::new();

	while let Some(Some(local)) = local.next().now_or_never() {
		if let moq_transfork::Announced::Active(track) = local {
			tracks.push(track.to_full());
		}
	}

	while let Some(Some(remote)) = remote.next().now_or_never() {
		if let moq_transfork::Announced::Active(track) = remote {
			tracks.push(track.to_full());
		}
	}

	tracks.join("\n")
}

/// Serve the latest group for a given track
async fn serve_fetch(Path(path): Path<String>, cluster: Cluster) -> axum::response::Result<ServeGroup> {
	let track = moq_transfork::Track {
		path,
		priority: 0,
		order: moq_transfork::GroupOrder::Desc,
	};

	tracing::info!(?track, "subscribing to track");

	let mut track = match cluster.router.subscribe(track).await {
		Ok(track) => track,
		Err(moq_transfork::Error::NotFound) => return Err(StatusCode::NOT_FOUND.into()),
		Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into()),
	};

	let group = match track.next_group().await {
		Ok(group) => group,
		Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into()),
	};

	let group = match group {
		Some(group) => group,
		None => return Err(StatusCode::NO_CONTENT.into()),
	};

	Ok(ServeGroup::new(group))
}

struct ServeGroup {
	group: moq_transfork::GroupConsumer,
	frame: Option<moq_transfork::FrameConsumer>,
}

impl ServeGroup {
	fn new(group: moq_transfork::GroupConsumer) -> Self {
		Self { group, frame: None }
	}

	async fn next(&mut self) -> moq_transfork::Result<Option<Bytes>> {
		loop {
			if let Some(frame) = self.frame.as_mut() {
				let data = frame.read_all().await?;
				self.frame.take();
				return Ok(Some(data));
			}

			self.frame = self.group.next_frame().await?;
			if self.frame.is_none() {
				return Ok(None);
			}
		}
	}
}

impl IntoResponse for ServeGroup {
	fn into_response(self) -> Response {
		Response::new(Body::new(self))
	}
}

impl http_body::Body for ServeGroup {
	type Data = Bytes;
	type Error = ServeGroupError;

	fn poll_frame(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
	) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
		let this = self.get_mut();

		// Use `poll_fn` to turn the async function into a Future
		let future = this.next();
		tokio::pin!(future);

		match ready!(future.poll(cx)) {
			Ok(Some(data)) => {
				let frame = http_body::Frame::data(data);
				Poll::Ready(Some(Ok(frame)))
			}
			Ok(None) => Poll::Ready(None),
			Err(e) => Poll::Ready(Some(Err(ServeGroupError(e)))),
		}
	}
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
struct ServeGroupError(moq_transfork::Error);

impl IntoResponse for ServeGroupError {
	fn into_response(self) -> Response {
		(StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()).into_response()
	}
}
