// A helper class to guard sending control messages behind a Mutex.

use std::{fmt, sync::Arc};

use tokio::sync::Mutex;
use webtransport_quinn::{RecvStream, SendStream};

use super::SessionError;
use crate::{message::Message, setup::Extensions};

#[derive(Debug, Clone)]
pub(crate) struct Control {
	send: Arc<Mutex<SendStream>>,
	recv: Arc<Mutex<RecvStream>>,
	pub ext: Extensions,
}

impl Control {
	pub fn new(send: SendStream, recv: RecvStream, ext: Extensions) -> Self {
		Self {
			send: Arc::new(Mutex::new(send)),
			recv: Arc::new(Mutex::new(recv)),
			ext,
		}
	}

	pub async fn send<T: Into<Message> + fmt::Debug>(&self, msg: T) -> Result<(), SessionError> {
		let mut stream = self.send.lock().await;
		log::info!("sending message: {:?}", msg);
		msg.into()
			.encode(&mut *stream, &self.ext)
			.await
			.map_err(|e| SessionError::Unknown(e.to_string()))?;
		Ok(())
	}

	// It's likely a mistake to call this from two different tasks, but it's easier to just support it.
	pub async fn recv(&self) -> Result<Message, SessionError> {
		let mut stream = self.recv.lock().await;
		let msg = Message::decode(&mut *stream, &self.ext)
			.await
			.map_err(|e| SessionError::Unknown(e.to_string()))?;
		Ok(msg)
	}
}
