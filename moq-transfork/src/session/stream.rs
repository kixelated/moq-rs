use super::{Close, Reader, Writer};
use crate::{message, Error};

pub(super) struct Stream {
	pub writer: Writer,
	pub reader: Reader,
}

impl Stream {
	pub async fn open(session: &mut web_transport::Session, typ: message::ControlType) -> Result<Self, Error> {
		let (send, recv) = session.open_bi().await?;

		let mut writer = Writer::new(send);
		let reader = Reader::new(recv);
		writer.encode(&typ).await?;

		Ok(Stream { writer, reader })
	}

	pub async fn accept(session: &mut web_transport::Session) -> Result<Self, Error> {
		let (send, recv) = session.accept_bi().await?;

		let writer = Writer::new(send);
		let reader = Reader::new(recv);

		Ok(Stream { writer, reader })
	}
}

impl Close<Error> for Stream {
	fn close(&mut self, err: Error) {
		self.writer.close(err.clone());
		self.reader.close(err);
	}
}
