use std::sync::Arc;

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum RelayError {
	#[error("session error: {0}")]
	Transport(#[from] moq_transport::SessionError),

	#[error("serve error: {0}")]
	Serve(#[from] moq_transport::serve::ServeError),

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
