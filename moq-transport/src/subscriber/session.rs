use webtransport_quinn::{RecvStream, SendStream};

use std::sync::{atomic, Arc};

use crate::{message, Error, Message, VarInt, Watch};

use tokio::{sync::mpsc, task::JoinSet};

use super::{Announce, Announces, Subscribe, Subscribes};

#[derive(Clone, Debug)]
pub struct Session {
	// The webtransport session.
	webtransport: webtransport_quinn::Session,

	// The list of active announces and subscriptions, guarded by an mutex.
	announces: Watch<Announces>,
	subscribes: Watch<Subscribes>,

	// We use an atomic so we don't need to grab the mutex twice on subscribe.
	subscribe_next: Arc<atomic::AtomicU32>,

	// A channel for sending messages.
	sender: mpsc::UnboundedSender<Message>,
}

impl Session {
	pub(crate) fn new(webtransport: webtransport_quinn::Session, control: (SendStream, RecvStream)) -> Self {
		let announces = Watch::new(Announces::new());
		let subscribes = Watch::new(Subscribes::new());
		let subscribe_next = Arc::new(atomic::AtomicU32::new(0));

		let (sender, receiver) = mpsc::unbounded_channel();

		let this = Self {
			webtransport,
			announces,
			subscribes,
			subscribe_next,
			sender,
		};

		tokio::spawn(this.clone().run(control, receiver));

		this
	}

	// Public API
	pub async fn announced(&mut self) -> Result<Announce, Error> {
		loop {
			let notify = {
				let state = self.announces.lock();

				if state.has_next()? {
					let next = state.as_mut().next()?;
					return Ok(next);
				}

				state.changed()
			};

			notify.await;
		}
	}

	pub fn subscribe(&mut self, namespace: &str, name: &str) -> Result<Subscribe, Error> {
		let id = self.subscribe_next.fetch_add(1, atomic::Ordering::Relaxed);
		let id = VarInt::from_u32(id);

		let subscribe = Subscribe::new(self.clone(), id, namespace, name);
		self.subscribes.lock_mut().insert(&subscribe)?;

		self.send(message::Subscribe {
			id,
			namespace: namespace.to_string(),
			name: name.to_string(),
		})?;

		Ok(subscribe)
	}

	pub fn close(self, code: u32, reason: &str) {
		self.webtransport.close(code, reason.as_bytes())
	}

	// Private API
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
			Message::Announce(msg) => {
				let announce = Announce::new(self.clone(), &msg.namespace);
				self.announces.lock_mut().insert(announce)
			}
			Message::AnnounceReset(msg) => self.announces.lock_mut().reset(msg),
			Message::SubscribeOk(msg) => self.subscribes.lock_mut().ok(msg),
			Message::SubscribeStop(msg) => self.subscribes.lock_mut().stop(msg),
			Message::GoAway(_msg) => unimplemented!("GOAWAY"),
			_ => Err(Error::RoleViolation(msg.id())),
		}
	}

	async fn run_outbound(
		self,
		mut control: SendStream,
		mut messages: mpsc::UnboundedReceiver<Message>,
	) -> Result<(), Error> {
		while let Some(msg) = messages.recv().await {
			msg.encode(&mut control).await.map_err(|_| Error::Unknown)?;

			log::info!("message sent: {:?}", msg);
		}

		Err(Error::Unknown)
	}

	async fn run_streams(self) -> Result<(), Error> {
		let mut streams = JoinSet::new();
		loop {
			tokio::select! {
				res = self.webtransport.accept_uni() => {
					let stream = res.map_err(|_e| Error::Unknown)?;
					streams.spawn(Self::run_stream(self.clone(), stream));
				},
				res = streams.join_next(), if !streams.is_empty() => {
					// Ignore stream errors
					res.expect("empty tasks").expect("aborted").ok();
				},
			}
		}
	}

	async fn run_stream(self, mut stream: RecvStream) -> Result<(), Error> {
		// TODO Add better errors to DecodeError.
		let obj = message::Object::decode(&mut stream).await.map_err(|_e| Error::Unknown)?;

		if let Err(err) = self.subscribes.lock_mut().object(&obj, stream) {
			log::warn!("object error: obj={:?} err={:?}", obj, err);
		}

		Ok(())
	}

	// Protected API

	pub(crate) fn send<T: Into<Message>>(&self, msg: T) -> Result<(), Error> {
		self.sender.send(msg.into()).map_err(|_e| Error::Unknown)
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
