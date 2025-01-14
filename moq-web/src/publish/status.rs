use baton::Baton;

#[derive(Debug, Default, Baton)]
pub struct Status {
	pub connected: bool,
	pub error: Option<String>,
}
