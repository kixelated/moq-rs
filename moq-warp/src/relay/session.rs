use super::{broker, contribute, control, distribute};

pub struct Session {
	// Split logic into contribution/distribution to reduce the problem space.
	contribute: contribute::Session,
	distribute: distribute::Session,

	// Used to receive control messages and forward to contribute/distribute.
	control: control::Main,
}

impl Session {
	pub fn new(session: moq_transport_quinn::Session, broker: broker::Broadcasts) -> Session {
		let (control, contribute, distribute) = control::split(session.send_control, session.recv_control);

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
