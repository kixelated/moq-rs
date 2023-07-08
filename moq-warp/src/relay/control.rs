use tokio::sync::mpsc;

use moq_transport::{Announce, AnnounceError, AnnounceOk, Message, Subscribe, SubscribeError, SubscribeOk};
use moq_transport_quinn::Control;

pub struct Main {
	control: Control,
	outgoing: mpsc::Receiver<Message>,

	contribute: mpsc::Sender<Contribute>,
	distribute: mpsc::Sender<Distribute>,
}

impl Main {
	pub async fn run(mut self) -> anyhow::Result<()> {
		loop {
			tokio::select! {
				Some(msg) = self.outgoing.recv() => self.control.send(msg).await?,
				Ok(msg) = self.control.recv() => self.handle(msg).await?,
			}
		}
	}

	pub async fn handle(&mut self, msg: Message) -> anyhow::Result<()> {
		match msg.try_into() {
			Ok(msg) => self.contribute.send(msg).await?,
			Err(msg) => match msg.try_into() {
				Ok(msg) => self.distribute.send(msg).await?,
				Err(msg) => anyhow::bail!("unsupported control message: {:?}", msg),
			},
		}

		Ok(())
	}
}

pub struct Component<T> {
	incoming: mpsc::Receiver<T>,
	outgoing: mpsc::Sender<Message>,
}

impl<T> Component<T> {
	pub async fn send<M: Into<Message>>(&mut self, msg: M) -> anyhow::Result<()> {
		self.outgoing.send(msg.into()).await?;
		Ok(())
	}

	pub async fn recv(&mut self) -> Option<T> {
		self.incoming.recv().await
	}
}

// Splits a control stream into two components, based on if it's a message for contribution or distribution.
pub fn split(control: Control) -> (Main, Component<Contribute>, Component<Distribute>) {
	let (outgoing_tx, outgoing_rx) = mpsc::channel(1);
	let (contribute_tx, contribute_rx) = mpsc::channel(1);
	let (distribute_tx, distribute_rx) = mpsc::channel(1);

	let control = Main {
		control,
		outgoing: outgoing_rx,
		contribute: contribute_tx,
		distribute: distribute_tx,
	};

	let contribute = Component {
		incoming: contribute_rx,
		outgoing: outgoing_tx.clone(),
	};

	let distribute = Component {
		incoming: distribute_rx,
		outgoing: outgoing_tx,
	};

	(control, contribute, distribute)
}

// Messages we expect to receive from the client for contribution.
#[derive(Debug)]
pub enum Contribute {
	Announce(Announce),
	SubscribeOk(SubscribeOk),
	SubscribeError(SubscribeError),
}

impl TryFrom<Message> for Contribute {
	type Error = Message;

	fn try_from(msg: Message) -> Result<Self, Self::Error> {
		match msg {
			Message::Announce(msg) => Ok(Self::Announce(msg)),
			Message::SubscribeOk(msg) => Ok(Self::SubscribeOk(msg)),
			Message::SubscribeError(msg) => Ok(Self::SubscribeError(msg)),
			_ => Err(msg),
		}
	}
}

// Messages we expect to receive from the client for distribution.
#[derive(Debug)]
pub enum Distribute {
	AnnounceOk(AnnounceOk),
	AnnounceError(AnnounceError),
	Subscribe(Subscribe),
}

impl TryFrom<Message> for Distribute {
	type Error = Message;

	fn try_from(value: Message) -> Result<Self, Self::Error> {
		match value {
			Message::AnnounceOk(msg) => Ok(Self::AnnounceOk(msg)),
			Message::AnnounceError(msg) => Ok(Self::AnnounceError(msg)),
			Message::Subscribe(msg) => Ok(Self::Subscribe(msg)),
			_ => Err(value),
		}
	}
}
