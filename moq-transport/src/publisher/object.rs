use crate::session::SessionError;

pub struct ObjectHeader {
	pub group_id: u64,
	pub object_id: u64,
	pub send_order: u64,
}

// Placeholder until there's an explicit size we should monitor.
pub struct ObjectWriter {
	stream: webtransport_quinn::SendStream,
}

impl ObjectWriter {
	pub(crate) fn new(stream: webtransport_quinn::SendStream) -> Self {
		Self { stream }
	}

	pub async fn write(&mut self, payload: &[u8]) -> Result<(), SessionError> {
		self.stream.write_all(payload).await?;
		Ok(())
	}
}
