use crate::relay::{contribute, distribute, message, Broker};

use webtransport_generic::{AsyncRecvStream, AsyncSendStream, AsyncSession};

pub struct Session<S>
where
	S: AsyncSession,
	S::SendStream: AsyncSendStream,
	S::RecvStream: AsyncRecvStream,
{
	// Split logic into contribution/distribution to reduce the problem space.
	contribute: contribute::Session<S>,
	distribute: distribute::Session<S>,

	// Used to receive control messages and forward to contribute/distribute.
	control: message::Main<S>,
}

impl<S> Session<S>
where
	S: AsyncSession,
	S::SendStream: AsyncSendStream,
	S::RecvStream: AsyncRecvStream,
{
	pub fn new(session: moq_transport::Session<S>, broker: Broker) -> Self {
		let (control, contribute, distribute) = message::split(session.send_control, session.recv_control);

		let contribute = contribute::Session::new(session.recv_objects, contribute, broker.clone());
		let distribute = distribute::Session::new(session.send_objects, distribute, broker);

		Self {
			control,
			contribute,
			distribute,
		}
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let control = self.control.run();
		let contribute = self.contribute.run();
		let distribute = self.distribute.run();

		tokio::try_join!(control, contribute, distribute)?;

		Ok(())
	}
}
