use std::sync::Arc;

use moq_transport::serve::ServeError;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum RelayError {
	#[error("session error: {0}")]
	Transport(#[from] moq_transport::SessionError),

	#[error("serve error: {0}")]
	Serve(#[from] ServeError),

	#[error("api error: {0}")]
	Api(#[from] Arc<moq_api::ApiError>),

	#[error("url error: {0}")]
	Url(#[from] url::ParseError),

	#[error("webtransport client error: {0}")]
	Client(#[from] webtransport_quinn::ClientError),

	#[error("webtransport server error: {0}")]
	Server(#[from] webtransport_quinn::ServerError),

	#[error("missing node")]
	MissingNode,
}

impl From<RelayError> for ServeError {
	fn from(err: RelayError) -> Self {
		match err {
			RelayError::Serve(err) => err,
			_ => ServeError::Internal(err.to_string()),
		}
	}
}
