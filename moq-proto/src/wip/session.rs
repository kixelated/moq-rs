use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, Encode},
	message::{self},
};

use super::{Error, StreamId};

#[derive(Debug)]
pub enum SessionEvent {
	Connected(message::ClientSetup, message::ServerSetup),
}

#[derive(Clone, Debug)]
pub struct Session {
	is_client: bool,
	client: Option<message::ClientSetup>,
	server: Option<message::ServerSetup>,
	stream: Option<StreamId>,
}

impl Session {
	pub(crate) fn new(is_client: bool) -> Self {
		Self {
			is_client,
			client: None,
			server: None,
			stream: None,
		}
	}

	pub(crate) fn encode<B: BufMut>(&mut self, buf: &mut B) {
		if self.is_client && self.client.is_none() {
			message::ControlType::Session.encode(buf);

			let client = message::ClientSetup {
				versions: [message::Version::CURRENT].into(),
				extensions: Default::default(),
			};
			client.encode(buf);

			self.client = Some(client);
		} else if !self.is_client && self.server.is_none() {
			// TODO utilize the client setup instead of blindly sending the current version
			let server = message::ServerSetup {
				version: message::Version::CURRENT,
				extensions: Default::default(),
			};

			server.encode(buf);
			self.server = Some(server);
		}
	}

	pub(crate) fn open(&mut self, stream: StreamId) -> bool {
		if self.stream.is_none() {
			self.stream = Some(stream);
			true
		} else {
			false
		}
	}

	pub(crate) fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		if !self.is_client && self.client.is_none() {
			let client = message::ClientSetup::decode(buf)?;
			if !client.versions.contains(&message::Version::CURRENT) {
				todo!("version error");
			}

			self.client = Some(client);
		} else if self.is_client && self.server.is_none() {
			let server = message::ServerSetup::decode(buf)?;
			if server.version != message::Version::CURRENT {
				todo!("version error");
			}

			self.server = Some(server);
		}

		Ok(())
	}

	pub(crate) fn accept<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<(), Error> {
		self.decode(buf)?;
		self.stream = Some(stream);

		Ok(())
	}
}
