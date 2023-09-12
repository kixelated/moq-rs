// A helper class to guard sending control messages behind a Mutex.

use std::sync::Arc;

use tokio::sync::Mutex;
use webtransport_quinn::SendStream;

use crate::{Error, Message};

#[derive(Debug, Clone)]
pub(crate) struct Control {
	stream: Arc<Mutex<SendStream>>,
}

impl Control {
	pub fn new(stream: SendStream) -> Self {
		Self {
			stream: Arc::new(Mutex::new(stream)),
		}
	}

	pub async fn send<T: Into<Message>>(&self, msg: T) -> Result<(), Error> {
		let mut stream = self.stream.lock().await;
		msg.into().encode(&mut *stream).await.map_err(|_e| Error::Unknown)
	}
}
