use moq_transport::control;
use tokio::sync::mpsc;

pub use control::*;

pub struct Main {
	control: control::Stream,
	outgoing: mpsc::Receiver<control::Message>,

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

	pub async fn handle(&mut self, msg: control::Message) -> anyhow::Result<()> {
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
	outgoing: mpsc::Sender<control::Message>,
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
pub fn split(control: control::Stream) -> (Main, Component<Contribute>, Component<Distribute>) {
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
	Announce(control::Announce),
	SubscribeOk(control::SubscribeOk),
	SubscribeError(control::SubscribeError),
}

impl TryFrom<control::Message> for Contribute {
	type Error = control::Message;

	fn try_from(msg: control::Message) -> Result<Self, Self::Error> {
		match msg {
			control::Message::Announce(msg) => Ok(Self::Announce(msg)),
			control::Message::SubscribeOk(msg) => Ok(Self::SubscribeOk(msg)),
			control::Message::SubscribeError(msg) => Ok(Self::SubscribeError(msg)),
			_ => Err(msg),
		}
	}
}

// Messages we expect to receive from the client for distribution.
#[derive(Debug)]
pub enum Distribute {
	AnnounceOk(control::AnnounceOk),
	AnnounceError(control::AnnounceError),
	Subscribe(control::Subscribe),
}

impl TryFrom<control::Message> for Distribute {
	type Error = control::Message;

	fn try_from(value: control::Message) -> Result<Self, Self::Error> {
		match value {
			control::Message::AnnounceOk(msg) => Ok(Self::AnnounceOk(msg)),
			control::Message::AnnounceError(msg) => Ok(Self::AnnounceError(msg)),
			control::Message::Subscribe(msg) => Ok(Self::Subscribe(msg)),
			_ => Err(value),
		}
	}
}
