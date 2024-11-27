use moq_karp::moq_transfork;
use moq_transfork::web_transport;
use wasm_bindgen::JsValue;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("transfork error: {0}")]
    Transfork(#[from] moq_transfork::Error),

    #[error("webtransport error: {0}")]
    WebTransport(#[from] web_transport::wasm::Error),

    #[error("webcodecs error: {0}")]
    WebCodecs(#[from] web_codecs::Error),

    #[error("karp error: {0}")]
    Karp(#[from] moq_karp::Error),

    #[error("invalid url")]
    InvalidUrl,

    #[error("invalid fingerprint")]
    InvalidFingerprint,

    #[error("offline")]
    Offline,

    #[error("http error: {0}")]
    Http(#[from] gloo_net::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<Error> for JsValue {
    fn from(err: Error) -> JsValue {
        JsValue::from_str(&format!("{}", err))
    }
}
