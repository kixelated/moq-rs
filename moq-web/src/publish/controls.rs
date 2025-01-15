use baton::Baton;
use url::Url;
use web_sys::{HtmlVideoElement, MediaStream};

#[derive(Debug, Default, Clone, Baton)]
pub struct Controls {
	pub url: Option<Url>,
	pub volume: f64,
	pub media: Option<MediaStream>,
	pub preview: Option<HtmlVideoElement>,
	pub close: bool,
}
