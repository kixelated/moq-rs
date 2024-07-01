use super::{Publisher, SessionError};
use crate::message;

#[derive(Debug, Clone)]
pub struct TrackStatusRequestedInfo {
    pub namespace: String,
    pub track: String,
}

pub struct TrackStatusRequested {
	publisher: Publisher,
	msg: message::TrackStatusRequest,
    pub info: TrackStatusRequestedInfo,
}

impl TrackStatusRequested {
    pub fn new(publisher: Publisher, msg: message::TrackStatusRequest) -> Self {
        Self { publisher, msg, info: TrackStatusRequestedInfo { namespace: msg.track_namespace.clone(), track: msg.track_name.clone() }}
    }

    pub async fn respond(&mut self, status: message::TrackStatus) -> Result<(), SessionError> {
        self.publisher.send_message(status);
        Ok(())
    }
}
