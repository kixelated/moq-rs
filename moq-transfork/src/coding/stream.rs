use crate::coding::{Reader, Writer};
use crate::message;
use crate::SessionError;

pub struct Stream {
	pub writer: Writer,
	pub reader: Reader,
}

impl Stream {
	pub async fn open(session: &mut web_transport::Session, typ: message::Control) -> Result<Self, SessionError> {
		let (send, recv) = session.open_bi().await?;
		let mut writer = Writer::new(send);
		let reader = Reader::new(recv);
		writer.encode(&typ).await?;
		Ok(Self { writer, reader })
	}

	pub async fn accept(session: &mut web_transport::Session) -> Result<(message::Control, Self), SessionError> {
		let (send, recv) = session.accept_bi().await?;
		let writer = Writer::new(send);
		let mut reader = Reader::new(recv);
		let typ = reader.decode().await?;
		Ok((typ, Self { writer, reader }))
	}
}
