use webtransport_quinn::{RecvStream, SendStream};

use crate::{control, error::SessionError, setup, util::Watch};

pub fn new(role: setup::Role) -> (Producer, Consumer) {
	let state = State::default();

	let session = Session::new(send, recv, role);
	let reader = Reader::new(role);
	(producer, reader)
}

pub struct Producer {
	role: setup::Role,
	recv: RecvStream,
	send: SendStream,
	state: Watch<State>,
}

impl Reader {
	pub fn new(role: setup::Role) -> Self {
		let state = Watch::new(State::default());
		Self { role, state }
	}

	async fn run(self, mut recv: RecvStream) -> Result<(), SessionError> {
		loop {
			self.run_wait().await?;
			self.run_next(&mut recv).await?;
		}
	}

	// Wait until the pending message has been read.
	async fn run_wait(&self) -> Result<(), SessionError> {
		loop {
			let notify = {
				let state = self.state.lock();
				state.closed.clone()?;

				if state.pending.is_none() {
					return Ok(())
				}

				state.changed()
			};

			notify.await;
		}
	}

	async fn run_next(&self, recv: &mut RecvStream) -> Result<(), SessionError> {
		let res = control::Message::decode(recv).await;
		let mut state = self.state.lock_mut();

		match res {
			Ok(msg) => state.pending = Some(msg),
			Err(err) => {
				state.closed = Err(err.clone().into()),
			},
		};

		Ok(())
	}


	async fn recv(&self) -> Result<control::Message, SessionError> {
		let state = self.state.lock();

		if let Some(msg) = state.pending.take() {
			return Ok(msg);
		}

		let msg = control::Message::decode(&mut state.recv).await?;
		Ok(msg)
	}

	// Return the next publisher message
	pub async fn recv_publisher(&self) -> Result<control::Message, SessionError> {
		let state = self.state.lock()

		self.publisher.as_mut()
	}

	// Return the next publisher message
	pub async fn recv_subscriber(&self) -> Result<control::Message, SessionError> {
		self.publisher.as_mut()
	}
}

#[derive(Default)]
pub struct State {
	pending: Option<control::Message>,
	closed: Result<(), SessionError>,
}
