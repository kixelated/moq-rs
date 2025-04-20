use std::sync::Arc;

use hang::moq_lite;
use moq_lite::web_transport;
use wasm_bindgen::JsValue;

#[derive(Clone, Debug, thiserror::Error)]
pub enum Error {
	#[error("moq error: {0}")]
	Moq(#[from] moq_lite::Error),

	#[error("webtransport error: {0}")]
	WebTransport(#[from] web_transport::Error),

	#[error("webcodecs error: {0}")]
	WebCodecs(#[from] web_codecs::Error),

	#[error("streams error: {0}")]
	Streams(#[from] web_streams::Error),

	#[error("karp error: {0}")]
	Karp(#[from] hang::Error),

	#[error("invalid url: {0}")]
	InvalidUrl(#[from] url::ParseError),

	#[error("invalid scheme")]
	InvalidScheme,

	#[error("invalid fingerprint")]
	InvalidFingerprint,

	#[error("offline")]
	Offline,

	#[error("unsupported")]
	Unsupported,

	#[error("closed")]
	Closed,

	#[error("capture failed")]
	InitFailed,

	#[error("http error: {0}")]
	Http(Arc<gloo_net::Error>),

	#[error("unclassified: {0}")]
	Js(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<Error> for JsValue {
	fn from(err: Error) -> JsValue {
		JsValue::from_str(&format!("{}", err))
	}
}

impl From<JsValue> for Error {
	fn from(value: JsValue) -> Self {
		Error::Js(value.as_string().unwrap_or_default())
	}
}

impl From<gloo_net::Error> for Error {
	fn from(err: gloo_net::Error) -> Self {
		Error::Http(Arc::new(err))
	}
}
