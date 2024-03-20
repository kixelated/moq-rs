use thiserror::Error;

#[derive(Error, Debug)]
pub enum RelayError {
	#[error("transport error: {0}")]
	Transport(#[from] moq_transport::session::SessionError),

	#[error("serve error: {0}")]
	Cache(#[from] moq_transport::serve::ServeError),

	#[error("api error: {0}")]
	MoqApi(#[from] moq_api::ApiError),

	#[error("url error: {0}")]
	Url(#[from] url::ParseError),

	#[error("webtransport client error: {0}")]
	WebTransportClient(#[from] webtransport_quinn::ClientError),

	#[error("webtransport server error: {0}")]
	WebTransportServer(#[from] webtransport_quinn::ServerError),

	#[error("missing node")]
	MissingNode,
}
