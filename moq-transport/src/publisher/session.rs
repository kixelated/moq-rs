use tokio::{sync::mpsc, task::JoinSet};
use webtransport_quinn::{RecvStream, SendStream};

use crate::{message, Error, Message, VarInt, Watch};

use super::{Announce, Announces, Subscribe, Subscribes};

#[derive(Clone, Debug)]
pub struct Session {
	announces: Watch<Announces>,
	subscribes: Watch<Subscribes>,

	webtransport: webtransport_quinn::Session,
	control: mpsc::UnboundedSender<Message>,
}

impl Session {
	pub fn new(webtransport: webtransport_quinn::Session, control: (SendStream, RecvStream)) -> Self {
		let announces = Watch::new(Announces::new());
		let subscribes = Watch::new(Subscribes::new());

		let (send, recv) = mpsc::unbounded_channel();

		let this = Self {
			webtransport,
			announces,
			subscribes,
			control: send,
		};

		tokio::spawn(this.clone().run(control, recv));

		this
	}

	pub fn announce(&mut self, namespace: &str) -> Result<Announce, Error> {
		let announce = Announce::new(self.clone(), namespace);
		self.announces.lock_mut().insert(&announce)?;

		self.send(message::Announce {
			namespace: announce.namespace().to_string(),
		})?;

		Ok(announce)
	}

	pub async fn subscribed(&mut self) -> Result<Subscribe, Error> {
		loop {
			let notify = {
				let subscribes = self.subscribes.lock();
				if subscribes.has_next()? {
					// Grab the mutable lock once we know there's a next.
					let next = subscribes.as_mut().next()?;
					return Ok(next);
				}

				subscribes.changed()
			};

			notify.await;
		}
	}

	pub fn close(self, code: u32) {
		self.webtransport.close(code, b"")
	}

	// Internal methods

	async fn run(
		self,
		control: (SendStream, RecvStream),
		messages: mpsc::UnboundedReceiver<Message>,
	) -> Result<(), Error> {
		let mut tasks = JoinSet::new();
		tasks.spawn(self.clone().run_inbound(control.1));
		tasks.spawn(self.clone().run_outbound(control.0, messages));
		tasks.spawn(self.clone().run_streams());

		let res = tasks
			.join_next()
			.await
			.expect("tasks were empty")
			.expect("tasks were aborted");

		let err = res.err().unwrap_or(Error::Unknown);

		// Close the announce and subscribe watches to unblock any waiting tasks.
		self.announces.lock_mut().close(err)?;
		self.subscribes.lock_mut().close(err)?;

		tasks.shutdown().await;

		Err(err)
	}

	async fn run_inbound(mut self, mut control: RecvStream) -> Result<(), Error> {
		loop {
			let msg = Message::decode(&mut control).await.map_err(|_e| Error::Unknown)?;

			log::info!("message received: {:?}", msg);
			if let Err(err) = self.recv_message(&msg) {
				log::warn!("message error: {:?} {:?}", err, msg);
			}
		}
	}

	fn recv_message(&mut self, msg: &Message) -> Result<(), Error> {
		match msg {
			Message::AnnounceOk(msg) => self.announces.lock_mut().ok(msg),
			Message::AnnounceStop(msg) => self.announces.lock_mut().stop(msg),
			Message::Subscribe(msg) => {
				let subscribe = Subscribe::new(self.clone(), &msg.namespace, &msg.name, msg.id);
				self.subscribes.lock_mut().insert(msg.id, subscribe)
			}
			Message::SubscribeReset(msg) => self.subscribes.lock_mut().reset(msg),
			_ => Err(Error::RoleViolation(msg.id())),
		}
	}

	async fn run_outbound(
		self,
		mut control: SendStream,
		mut messages: mpsc::UnboundedReceiver<Message>,
	) -> Result<(), Error> {
		while let Some(msg) = messages.recv().await {
			msg.encode(&mut control).await.map_err(|_e| Error::Unknown)?;

			log::info!("message sent: {:?}", msg);
		}

		Err(Error::Unknown)
	}

	// Literally error if we receive a unidirectional QUIC stream.
	async fn run_streams(self) -> Result<(), Error> {
		if self.webtransport.accept_uni().await.is_ok() {
			return Err(Error::RoleViolation(VarInt::from_u32(0)));
		}

		Ok(())
	}

	pub(crate) fn send<T: Into<Message>>(&self, msg: T) -> Result<(), Error> {
		// TODO Add better typing to DecodeError so we can return the correct error messages.
		self.control.send(msg.into()).map_err(|_e| Error::Unknown)
	}

	pub(crate) fn webtransport(&self) -> &webtransport_quinn::Session {
		&self.webtransport
	}

	pub(crate) fn unannounce(&self, namespace: &str) -> Result<(), Error> {
		self.announces.lock_mut().remove(namespace)?;
		Ok(())
	}

	pub(crate) fn unsubscribe(&self, id: VarInt) -> Result<(), Error> {
		self.subscribes.lock_mut().remove(id)?;
		Ok(())
	}
}
