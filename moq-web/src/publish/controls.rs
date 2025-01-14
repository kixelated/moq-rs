use baton::Baton;
use web_sys::MediaStream;

#[derive(Debug, Default, Baton)]
pub struct Controls {
	pub volume: f64,
	pub media: Option<MediaStream>,
	pub close: bool,
}
