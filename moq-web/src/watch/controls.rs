use baton::Baton;
use url::Url;
use web_sys::HtmlCanvasElement;

#[derive(Debug, Default, Baton)]
pub struct Controls {
	pub url: Option<Url>,
	pub paused: bool,
	pub volume: f64,
	pub canvas: Option<HtmlCanvasElement>,
	pub close: bool,
}
