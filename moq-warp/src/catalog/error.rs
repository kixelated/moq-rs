#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("json error: {0}")]
	Json(#[from] serde_json::Error),

	#[error("moq error: {0}")]
	Moq(#[from] moq_transfork::MoqError),

	#[error("empty catalog")]
	Empty,
}

pub type Result<T> = std::result::Result<T, Error>;
