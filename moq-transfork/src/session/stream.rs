use crate::message;
use crate::SessionError;

use super::{Close, Reader, Writer};

pub struct Stream {
	pub writer: Writer,
	pub reader: Reader,
}

impl Stream {
	pub async fn open(session: &mut web_transport::Session, typ: message::Stream) -> Result<Self, SessionError> {
		let (send, recv) = session.open_bi().await?;

		let mut writer = Writer::new(send);
		let reader = Reader::new(recv);
		writer.encode_silent(&typ).await?;
		Ok(Self { writer, reader })
	}

	pub async fn accept(session: &mut web_transport::Session) -> Result<Self, SessionError> {
		let (send, recv) = session.accept_bi().await?;
		let writer = Writer::new(send);
		let reader = Reader::new(recv);
		Ok(Self { writer, reader })
	}
}

impl Close for Stream {
	fn close(&mut self, code: u32) {
		self.writer.close(code);
		self.reader.close(code);
	}
}
