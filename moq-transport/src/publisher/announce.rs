use crate::session::SessionError;

pub struct Announce {}

impl Announce {
	pub async fn closed() -> Result<(), SessionError> {
		unimplemented!("closed")
	}

	pub fn close(mut self, code: u64, reason: String) {
		unimplemented!("close")
	}
}

impl Drop for Announce {
	fn drop(&mut self) {
		self.close(0, "dropped".to_string());
	}
}
