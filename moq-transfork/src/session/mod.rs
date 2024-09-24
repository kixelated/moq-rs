use crate::{
	async_clone, message, setup,
	util::{spawn, Close, OrClose},
	Error,
};
mod announced;
mod client;
mod publisher;
mod reader;
mod server;
mod stream;
mod subscribe;
mod subscriber;
mod writer;

pub use announced::*;
pub use client::*;
pub use publisher::*;
pub use server::*;
pub use subscriber::*;

use reader::*;
use stream::*;
use subscribe::*;
use writer::*;

use tokio::sync::watch;

struct SessionState {
	closed: Result<(), Error>,
}

impl Default for SessionState {
	fn default() -> Self {
		Self { closed: Ok(()) }
	}
}

#[derive(Clone)]
pub(crate) struct Session {
	webtransport: web_transport::Session,
	state: watch::Sender<SessionState>,
}

impl Session {
	pub fn new(webtransport: web_transport::Session) -> Self {
		Self {
			webtransport,
			state: Default::default(),
		}
	}

	pub fn start(self, role: setup::Role, stream: Stream) -> (Option<Publisher>, Option<Subscriber>) {
		let publisher = role.is_publisher().then(|| Publisher::new(self.clone()));
		let subscriber = role.is_subscriber().then(|| Subscriber::new(self.clone()));
		let this = self;

		spawn(async_clone!(this, {
			this.run_session(stream).await.or_close(&mut this).ok();
		}));

		spawn(async_clone!(this, publisher, subscriber, {
			this.run_bi(publisher, subscriber).await.or_close(&mut this).ok();
		}));

		spawn(async_clone!(this, subscriber, {
			this.run_uni(subscriber).await.or_close(&mut this).ok();
		}));

		(publisher, subscriber)
	}

	async fn run_session(&mut self, mut stream: Stream) -> Result<(), Error> {
		while let Some(_info) = stream.reader.decode_maybe::<setup::Info>().await? {}
		Err(Error::Cancel)
	}

	async fn run_uni(&mut self, subscriber: Option<Subscriber>) -> Result<(), Error> {
		loop {
			let mut stream = self.accept_uni().await?;
			let mut subscriber = subscriber.clone().ok_or(Error::RoleViolation)?;

			spawn(async move {
				match stream.decode().await {
					Ok(message::StreamUni::Group) => subscriber.recv_group(stream).await,
					Err(err) => stream.close(err),
				};
			});
		}
	}

	async fn run_bi(&mut self, publisher: Option<Publisher>, subscriber: Option<Subscriber>) -> Result<(), Error> {
		loop {
			let mut stream = self.accept().await?;
			let publisher = publisher.clone();
			let subscriber = subscriber.clone();

			spawn(async move {
				Self::run_control(&mut stream, publisher, subscriber)
					.await
					.or_close(&mut stream)
					.ok();
			});
		}
	}

	async fn run_control(
		stream: &mut Stream,
		publisher: Option<Publisher>,
		subscriber: Option<Subscriber>,
	) -> Result<(), Error> {
		let kind = stream.reader.decode().await?;
		match kind {
			message::Stream::Session => Err(Error::UnexpectedStream(kind)),
			message::Stream::Announce => {
				let mut subscriber = subscriber.ok_or(Error::RoleViolation)?;
				subscriber.recv_announce(stream).await
			}
			message::Stream::Subscribe => {
				let mut publisher = publisher.ok_or(Error::RoleViolation)?;
				publisher.recv_subscribe(stream).await
			}
			message::Stream::Fetch => {
				let mut publisher = publisher.ok_or(Error::RoleViolation)?;
				publisher.recv_fetch(stream).await
			}
			message::Stream::Info => {
				let mut publisher = publisher.ok_or(Error::RoleViolation)?;
				publisher.recv_info(stream).await
			}
		}
	}

	pub async fn open(&mut self, typ: message::Stream) -> Result<Stream, Error> {
		let (send, recv) = self.webtransport.open_bi().await?;

		let mut writer = Writer::new(send);
		let reader = Reader::new(recv);
		writer.encode(&typ).await?;

		Ok(Stream { writer, reader })
	}

	pub async fn open_uni(&mut self, typ: message::StreamUni) -> Result<Writer, Error> {
		let send = self.webtransport.open_uni().await?;

		let mut writer = Writer::new(send);
		writer.encode(&typ).await?;

		Ok(writer)
	}

	pub async fn accept(&mut self) -> Result<Stream, Error> {
		let (send, recv) = self.webtransport.accept_bi().await?;

		let writer = Writer::new(send);
		let reader = Reader::new(recv);

		Ok(Stream { writer, reader })
	}

	pub async fn accept_uni(&mut self) -> Result<Reader, Error> {
		let stream = self.webtransport.accept_uni().await?;
		Ok(Reader::new(stream))
	}

	pub async fn closed(&self) -> Result<(), Error> {
		let mut state = self.state.subscribe();
		loop {
			state.borrow_and_update().closed.clone()?;
			if state.changed().await.is_err() {
				return Ok(());
			}
		}
	}
}

impl Close for Session {
	fn close(&mut self, err: Error) {
		tracing::warn!(?err, "closing session");
		self.webtransport.close(err.to_code(), &err.to_string());
		self.state.send_modify(|state| state.closed = Err(err));
	}
}
