use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
	#[error("redis error: {0}")]
	Redis(#[from] redis::RedisError),

	#[error("reqwest error: {0}")]
	Request(#[from] reqwest::Error),

	#[error("hyper error: {0}")]
	Hyper(#[from] hyper::Error),

	#[error("url error: {0}")]
	Url(#[from] url::ParseError),
}
