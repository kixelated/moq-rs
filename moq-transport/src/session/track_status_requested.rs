use super::{Publisher, SessionError};
use crate::message;

#[derive(Debug, Clone)]
pub struct TrackStatusRequestedInfo {
	pub namespace: String,
	pub track: String,
}

pub struct TrackStatusRequested {
	publisher: Publisher,
	// msg: message::TrackStatusRequest, // TODO: See if we actually need this
	pub info: TrackStatusRequestedInfo,
}

impl TrackStatusRequested {
	pub fn new(publisher: Publisher, msg: message::TrackStatusRequest) -> Self {
		let namespace = msg.track_namespace.clone();
		let track = msg.track_name.clone();
		Self {
			publisher,
			info: TrackStatusRequestedInfo { namespace, track },
		}
	}

	pub async fn respond(&mut self, status: message::TrackStatus) -> Result<(), SessionError> {
		self.publisher.send_message(status);
		Ok(())
	}
}
