use crate::{
	async_clone, message,
	runtime::{self, Watch},
	setup,
	util::{Close, OrClose},
	MoqError,
};
mod client;
mod publisher;
mod reader;
mod server;
mod stream;
mod subscriber;
mod writer;

pub use client::*;
pub use publisher::*;
pub use server::*;
pub use subscriber::*;

use reader::*;
use stream::*;
use writer::*;

struct SessionState {
	closed: Result<(), MoqError>,
}

impl Default for SessionState {
	fn default() -> Self {
		Self { closed: Ok(()) }
	}
}

#[derive(Clone)]
pub(crate) struct Session {
	webtransport: web_transport::Session,
	state: Watch<SessionState>,
}

impl Session {
	pub fn new(webtransport: web_transport::Session) -> Self {
		Self {
			webtransport,
			state: Default::default(),
		}
	}

	pub fn start(self, role: setup::Role, stream: Stream) -> (Option<Publisher>, Option<Subscriber>) {
		let backend = self.split();

		let publisher = role.is_publisher().then(|| Publisher::new(self.clone()));
		let subscriber = role.is_subscriber().then(|| Subscriber::new(self));

		runtime::spawn(async_clone!(backend, {
			backend.run_session(stream).await.or_close(&mut backend).ok();
		}));

		runtime::spawn(async_clone!(backend, publisher, subscriber, {
			backend.run_bi(publisher, subscriber).await.or_close(&mut backend).ok();
		}));

		runtime::spawn(async_clone!(backend, subscriber, {
			backend.run_uni(subscriber).await.or_close(&mut backend).ok();
		}));

		(publisher, subscriber)
	}

	async fn run_session(&mut self, mut stream: Stream) -> Result<(), MoqError> {
		while let Some(_info) = stream.reader.decode_maybe::<setup::Info>().await? {}
		Err(MoqError::Cancel.into())
	}

	async fn run_uni(&mut self, subscriber: Option<Subscriber>) -> Result<(), MoqError> {
		loop {
			let mut stream = self.accept_uni().await?;
			let subscriber = subscriber.clone().ok_or(MoqError::RoleViolation)?;

			runtime::spawn(async move {
				Self::run_data(&mut stream, subscriber).await.or_close(&mut stream).ok();
			});
		}
	}

	async fn run_bi(&mut self, publisher: Option<Publisher>, subscriber: Option<Subscriber>) -> Result<(), MoqError> {
		loop {
			let mut stream = self.accept().await?;
			let publisher = publisher.clone();
			let subscriber = subscriber.clone();

			runtime::spawn(async move {
				Self::run_control(&mut stream, publisher, subscriber)
					.await
					.or_close(&mut stream)
					.ok();
			});
		}
	}

	async fn run_data(stream: &mut Reader, mut subscriber: Subscriber) -> Result<(), MoqError> {
		match stream.decode_silent().await? {
			message::StreamUni::Group => subscriber.recv_group(stream).await,
		}
	}

	async fn run_control(
		stream: &mut Stream,
		publisher: Option<Publisher>,
		subscriber: Option<Subscriber>,
	) -> Result<(), MoqError> {
		let kind = stream.reader.decode_silent().await?;
		match kind {
			message::Stream::Session => return Err(MoqError::UnexpectedStream(kind)),
			message::Stream::Announce => {
				let mut subscriber = subscriber.ok_or(MoqError::RoleViolation)?;
				subscriber.recv_announce(stream).await
			}
			message::Stream::Subscribe => {
				let mut publisher = publisher.ok_or(MoqError::RoleViolation)?;
				publisher.recv_subscribe(stream).await
			}
			message::Stream::Datagrams => {
				let mut publisher = publisher.ok_or(MoqError::RoleViolation)?;
				publisher.recv_datagrams(stream).await
			}
			message::Stream::Fetch => {
				let mut publisher = publisher.ok_or(MoqError::RoleViolation)?;
				publisher.recv_fetch(stream).await
			}
			message::Stream::Info => {
				let mut publisher = publisher.ok_or(MoqError::RoleViolation)?;
				publisher.recv_info(stream).await
			}
		}
	}

	pub async fn open(&mut self, typ: message::Stream) -> Result<Stream, MoqError> {
		let (send, recv) = self.webtransport.open_bi().await?;

		let mut writer = Writer::new(send);
		let reader = Reader::new(recv);
		writer.encode_silent(&typ).await?;

		Ok(Stream { writer, reader })
	}

	pub async fn open_uni(&mut self, typ: message::StreamUni) -> Result<Writer, MoqError> {
		let send = self.webtransport.open_uni().await?;

		let mut writer = Writer::new(send);
		writer.encode_silent(&typ).await?;

		Ok(writer)
	}

	pub async fn accept(&mut self) -> Result<Stream, MoqError> {
		let (send, recv) = self.webtransport.accept_bi().await?;
		let writer = Writer::new(send);
		let reader = Reader::new(recv);
		Ok(Stream { writer, reader })
	}

	pub async fn accept_uni(&mut self) -> Result<Reader, MoqError> {
		let recv = self.webtransport.accept_uni().await?;
		let reader = Reader::new(recv);
		Ok(reader)
	}

	pub async fn closed(&self) -> Result<(), MoqError> {
		loop {
			{
				let state = self.state.lock();
				state.closed.clone()?;

				match state.changed() {
					Some(notify) => notify,
					None => return Err(MoqError::Cancel.into()),
				}
			}
			.await;
		}
	}

	pub fn split(&self) -> Self {
		Self {
			webtransport: self.webtransport.clone(),
			state: self.state.split(),
		}
	}
}

impl Close for Session {
	fn close(&mut self, err: MoqError) {
		if let Some(mut state) = self.state.lock_mut() {
			tracing::warn!(?err, "closing session");
			self.webtransport.close(err.to_code(), &err.to_string());
			state.closed = Err(err);
		}
	}
}
