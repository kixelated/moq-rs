use std::{fmt, ops};

use crate::coding::{Reader, Writer};
use crate::message;
use crate::SessionError;

pub struct Stream {
	pub writer: Writer,
	pub reader: Reader,
	pub info: web_transport::StreamInfo,
}

impl Stream {
	pub async fn open(session: &mut web_transport::Session, typ: message::Control) -> Result<Self, SessionError> {
		let (send, recv) = session.open_bi().await?;
		let info = send.info.clone();
		let mut writer = Writer::new(send);
		let reader = Reader::new(recv);
		writer.encode(&typ).await?;
		Ok(Self { info, writer, reader })
	}

	pub async fn accept(session: &mut web_transport::Session) -> Result<(message::Control, Self), SessionError> {
		let (send, recv) = session.accept_bi().await?;
		let info = send.info.clone();
		let writer = Writer::new(send);
		let mut reader = Reader::new(recv);
		let typ = reader.decode().await?;
		Ok((typ, Self { info, writer, reader }))
	}

	pub fn close(&mut self, code: u32) {
		self.writer.reset(code);
		self.reader.stop(code);
	}
}

impl ops::Deref for Stream {
	type Target = web_transport::StreamInfo;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
