use baton::Baton;
use url::Url;

#[derive(Debug, Default, Baton)]
pub struct Controls {
	pub url: Option<Url>,
	pub paused: bool,
	pub volume: f64,
	pub canvas: Option<web_sys::OffscreenCanvas>,
	pub close: bool,
}
