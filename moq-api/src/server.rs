use std::net;

use axum::{
	extract::{Path, State},
	http::StatusCode,
	response::{IntoResponse, Response},
	routing::get,
	Json, Router,
};
use clap::Parser;

use redis::{aio::ConnectionManager, AsyncCommands};

use anyhow::Context;
use uuid::Uuid;

use super::{Broadcast, Broadcasts};

/// Runs a HTTP API to create/get origins for broadcasts.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct ServerConfig {
	/// Bind to the given address
	#[arg(long)]
	pub bind: net::SocketAddr,

	/// Connect to the given redis instance
	#[arg(long)]
	pub redis: url::Url,
}

pub struct Server {
	config: ServerConfig,
}

impl Server {
	pub fn new(config: ServerConfig) -> Self {
		Self { config }
	}

	pub async fn run(self) -> anyhow::Result<()> {
		// Create the redis client.
		let redis = redis::Client::open(self.config.redis).context("failed to create redis client")?;
		let redis = redis
			.get_tokio_connection_manager() // TODO get_tokio_connection_manager_with_backoff?
			.await
			.context("failed to get async redis connection")?;

		let app = Router::new()
			.route("/broadcasts", get(get_broadcasts).post(create_broadcast))
			.route(
				"/broadcast/:id",
				get(get_broadcast).post(set_broadcast).delete(delete_broadcast),
			)
			.with_state(redis);

		axum::Server::bind(&self.config.bind)
			.serve(app.into_make_service())
			.await?;

		Ok(())
	}
}

async fn get_broadcasts(State(_redis): State<ConnectionManager>) -> Json<Broadcasts> {
	let broadcasts = Default::default();
	Json(broadcasts)
}

async fn create_broadcast(
	State(mut redis): State<ConnectionManager>,
	Json(broadcast): Json<Broadcast>,
) -> Result<Json<Uuid>, AppError> {
	// TODO validate broadcast

	let id = Uuid::new_v4();
	let key = broadcast_key(&id);

	// Convert the input back to JSON after validating it add adding any fields (TODO)
	let payload = serde_json::to_string(&broadcast)?;

	let res: Option<String> = redis::cmd("SET")
		.arg(key)
		.arg(payload)
		.arg("NX")
		.arg("EX")
		.arg(60 * 60 * 24 * 2) // Set the key to expire in 2 days; just in case we forget to remove it.
		.query_async(&mut redis)
		.await?;

	if res.is_none() {
		return Err(AppError::Duplicate);
	}

	Ok(Json(id))
}

async fn get_broadcast(
	Path(id): Path<Uuid>,
	State(mut redis): State<ConnectionManager>,
) -> Result<Json<Broadcast>, AppError> {
	let key = broadcast_key(&id);

	let payload: String = match redis.get(&key).await? {
		Some(payload) => payload,
		None => return Err(AppError::NotFound),
	};

	let broadcast: Broadcast = serde_json::from_str(&payload)?;

	Ok(Json(broadcast))
}

async fn set_broadcast(
	State(mut redis): State<ConnectionManager>,
	Path(id): Path<Uuid>,
	Json(broadcast): Json<Broadcast>,
) -> Result<(), AppError> {
	// TODO validate broadcast

	let key = broadcast_key(&id);

	// Convert the input back to JSON after validating it add adding any fields (TODO)
	let payload = serde_json::to_string(&broadcast)?;

	let res: Option<String> = redis::cmd("SET")
		.arg(key)
		.arg(payload)
		.arg("NX")
		.arg("EX")
		.arg(60 * 60 * 24 * 2) // Set the key to expire in 2 days; just in case we forget to remove it.
		.query_async(&mut redis)
		.await?;

	if res.is_none() {
		return Err(AppError::Duplicate);
	}

	Ok(())
}

async fn delete_broadcast(Path(id): Path<Uuid>, State(mut redis): State<ConnectionManager>) -> Result<(), AppError> {
	let key = broadcast_key(&id);
	match redis.del(key).await? {
		0 => Err(AppError::NotFound),
		_ => Ok(()),
	}
}

fn broadcast_key(id: &Uuid) -> String {
	format!("broadcast.{}", id.hyphenated())
}

#[derive(thiserror::Error, Debug)]
enum AppError {
	#[error("redis error")]
	Redis(#[from] redis::RedisError),

	#[error("json error")]
	Json(#[from] serde_json::Error),

	#[error("not found")]
	NotFound,

	#[error("duplicate ID")]
	Duplicate,
}

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
	fn into_response(self) -> Response {
		match self {
			AppError::Redis(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("redis error: {}", e)).into_response(),
			AppError::Json(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("json error: {}", e)).into_response(),
			AppError::NotFound => StatusCode::NOT_FOUND.into_response(),
			AppError::Duplicate => StatusCode::CONFLICT.into_response(),
		}
	}
}
