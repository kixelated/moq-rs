use thiserror::Error;

#[derive(Error, Debug)]
pub enum RelayError {
	#[error("transport error: {0}")]
	Transport(#[from] moq_transport::session::SessionError),

	#[error("cache error: {0}")]
	Cache(#[from] moq_transport::cache::CacheError),

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

impl moq_transport::MoqError for RelayError {
	fn code(&self) -> u32 {
		match self {
			Self::Transport(err) => err.code(),
			Self::Cache(err) => err.code(),
			Self::MoqApi(_err) => 504,
			Self::Url(_) => 500,
			Self::MissingNode => 500,
			Self::WebTransportClient(_) => 504,
			Self::WebTransportServer(_) => 500,
		}
	}

	fn reason(&self) -> String {
		match self {
			Self::Transport(err) => format!("transport error: {}", err.reason()),
			Self::Cache(err) => format!("cache error: {}", err.reason()),
			Self::MoqApi(err) => format!("api error: {}", err),
			Self::Url(err) => format!("url error: {}", err),
			Self::MissingNode => "missing node".to_owned(),
			Self::WebTransportServer(err) => format!("upstream server error: {}", err),
			Self::WebTransportClient(err) => format!("upstream client error: {}", err),
		}
	}
}
