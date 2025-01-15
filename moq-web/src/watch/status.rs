use baton::Baton;

use super::WatchState;
use crate::Error;

#[derive(Debug, Default, Baton)]
pub struct Status {
	pub state: WatchState,
	pub error: Option<Error>,
}
