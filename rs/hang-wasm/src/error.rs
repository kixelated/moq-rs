use hang::moq_lite;
use wasm_bindgen::JsValue;

use crate::ConnectError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("moq error: {0}")]
	Moq(#[from] moq_lite::Error),

	#[error("webcodecs error: {0}")]
	WebCodecs(#[from] web_codecs::Error),

	#[error("streams error: {0}")]
	Streams(#[from] web_streams::Error),

	#[error("karp error: {0}")]
	Karp(#[from] hang::Error),

	#[error("offline")]
	Offline,

	#[error("unsupported")]
	Unsupported,

	#[error("closed")]
	Closed,

	#[error("capture failed")]
	InitFailed,

	#[error("no broadcast")]
	NoBroadcast,

	#[error("no catalog")]
	NoCatalog,

	#[error("no track")]
	NoTrack,

	#[error("not visible")]
	NotVisible,

	#[error("invalid dimensions")]
	InvalidDimensions,

	#[error("unclassified: {0}")]
	Js(String),

	#[error("connect error: {0}")]
	Connect(#[from] ConnectError),

	#[error("resampler init: {0}")]
	ResamplerInit(#[from] rubato::ResamplerConstructionError),

	#[error("resampler: {0}")]
	Resampler(#[from] rubato::ResampleError),
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
