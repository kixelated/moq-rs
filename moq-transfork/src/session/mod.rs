use std::future;

use crate::{message, runtime, runtime::Watch, setup, Closed};
use futures::{stream::FuturesUnordered, StreamExt};

mod announce;
mod client;
mod error;
mod publisher;
mod reader;
mod server;
mod stream;
mod subscribe;
mod subscriber;
mod writer;

pub use client::*;
pub use error::*;
pub use publisher::*;
pub use server::*;
pub use subscriber::*;

use announce::*;
use reader::*;
use stream::*;
use subscribe::*;
use writer::*;

struct SessionState {
	closed: Result<(), SessionError>,
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

	pub fn spawn(self, role: setup::Role, stream: Stream) -> (Option<Publisher>, Option<Subscriber>) {
		let frontend = self.split();

		let publisher = role.is_publisher().then(|| Publisher::new(frontend.clone()));
		let subscriber = role.is_subscriber().then(|| Subscriber::new(frontend));

		let background = Background {
			session: self,
			setup: stream,
			publisher: publisher.clone(),
			subscriber: subscriber.clone(),
		};

		runtime::spawn(background.run());

		(publisher, subscriber)
	}

	pub async fn open(&mut self, typ: message::Stream) -> Result<Stream, SessionError> {
		let (send, recv) = self.webtransport.open_bi().await?;

		let mut writer = Writer::new(send);
		let reader = Reader::new(recv);
		writer.encode_silent(&typ).await?;

		Ok(Stream { writer, reader })
	}

	pub async fn open_uni(&mut self, typ: message::StreamUni) -> Result<Writer, SessionError> {
		let send = self.webtransport.open_uni().await?;

		let mut writer = Writer::new(send);
		writer.encode_silent(&typ).await?;

		Ok(writer)
	}

	pub async fn accept(&mut self) -> Result<Stream, SessionError> {
		let (send, recv) = self.webtransport.accept_bi().await?;
		let writer = Writer::new(send);
		let reader = Reader::new(recv);
		Ok(Stream { writer, reader })
	}

	pub async fn accept_uni(&mut self) -> Result<Reader, SessionError> {
		let recv = self.webtransport.accept_uni().await?;
		let reader = Reader::new(recv);
		Ok(reader)
	}

	pub fn close(&self, err: SessionError) {
		if let Some(mut state) = self.state.lock_mut() {
			state.closed = Err(err);
		}
	}

	pub async fn closed(&self) -> Result<(), SessionError> {
		loop {
			{
				let state = self.state.lock();
				state.closed.clone()?;

				match state.changed() {
					Some(notify) => notify,
					None => return Err(Closed::Unknown.into()),
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

struct Background {
	session: Session,
	setup: Stream,
	publisher: Option<Publisher>,
	subscriber: Option<Subscriber>,
}

impl Background {
	async fn run(mut self) {
		let res = tokio::select! {
			res = Self::run_update(&mut self.setup) => res,
			res = Self::run_bi(self.session.clone(), self.publisher.clone(), self.subscriber.clone()) => res,
			res = Self::run_uni(self.session.clone(), self.subscriber.clone()) => res,
			res = async move {
				match self.publisher {
					Some(publisher) => publisher.run().await,
					None => future::pending().await,
				}
			} => res,
			res = async move {
				match self.subscriber {
					Some(subscriber) => subscriber.run().await,
					None => future::pending().await,
				}
			} => res,
			res = self.session.closed() => res,
		}
		.or_close(&mut self.setup);

		if let Err(err) = res {
			tracing::warn!(?err, "session closed");
			self.session.close(err);
		}
	}

	async fn run_update(stream: &mut Stream) -> Result<(), SessionError> {
		while let Some(_update) = stream.reader.decode_maybe::<setup::Info>().await? {}

		Ok(())
	}

	async fn run_uni(mut session: Session, subscriber: Option<Subscriber>) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = session.accept_uni() => {
					let stream = res?;
					let subscriber = subscriber.clone().ok_or(SessionError::RoleViolation)?;

					tasks.push(Self::run_data(stream, subscriber));
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?,
			};
		}
	}

	async fn run_bi(
		mut session: Session,
		publisher: Option<Publisher>,
		subscriber: Option<Subscriber>,
	) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = session.accept() => {
					let stream = res?;
					let publisher = publisher.clone();
					let subscriber = subscriber.clone();

					tasks.push(Self::run_control(stream, publisher, subscriber));
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?,
			};
		}
	}

	async fn run_data(mut stream: Reader, mut subscriber: Subscriber) -> Result<(), SessionError> {
		match stream.decode_silent().await? {
			message::StreamUni::Group => subscriber.recv_group(&mut stream).await,
		}
		.or_close(&mut stream)
		.ok();

		Ok(())
	}

	async fn run_control(
		mut stream: Stream,
		publisher: Option<Publisher>,
		subscriber: Option<Subscriber>,
	) -> Result<(), SessionError> {
		let kind = stream.reader.decode_silent().await?;
		match kind {
			message::Stream::Session => return Err(SessionError::UnexpectedStream(kind)),
			message::Stream::Announce => {
				let mut subscriber = subscriber.ok_or(SessionError::RoleViolation)?;
				subscriber.recv_announce(&mut stream).await
			}
			message::Stream::Subscribe => {
				let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
				publisher.recv_subscribe(&mut stream).await
			}
			message::Stream::Datagrams => {
				let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
				publisher.recv_datagrams(&mut stream).await
			}
			message::Stream::Fetch => {
				let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
				publisher.recv_fetch(&mut stream).await
			}
			message::Stream::Info => {
				let mut publisher = publisher.ok_or(SessionError::RoleViolation)?;
				publisher.recv_info(&mut stream).await
			}
		}
		.or_close(&mut stream)
		.ok();

		Ok(())
	}
}
