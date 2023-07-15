use std::marker::PhantomData;

use webtransport_generic::{SendStream, Connection, RecvStream};

use super::{broker, contribute, control, distribute};


pub struct Session<R: RecvStream + Send, S: SendStream + Send, C: Connection + Send> {
	// Split logic into contribution/distribution to reduce the problem space.
	contribute: contribute::Session<R, C>,
	distribute: distribute::Session<S, C>,

	// Used to receive control messages and forward to contribute/distribute.
	control: control::Main<S, R>,
	_marker: PhantomData<S>,
	_marker_r: PhantomData<R>,
}

impl<R, S, C> Session<R, S, C> where
	R: RecvStream + Send + 'static,
	S: SendStream + Send,
	C: Connection<RecvStream = R, SendStream = S> + Send + 'static
{
	pub async fn from_transport_session(
		session: moq_transport::Session<C>,
		broker: broker::Broadcasts,
	) -> anyhow::Result<Session<R, S, C>> {
		let (control, objects) = session.split();
		let (objects_send, objects_recv) = objects.split();

		let (control, contribute, distribute) = control::split(control);

		let contribute = contribute::Session::new(objects_recv, contribute, broker.clone());
		let distribute = distribute::Session::new(objects_send, distribute, broker);

		let session = Self {
			control,
			contribute,
			distribute,
    		_marker: PhantomData,
    		_marker_r: PhantomData,
		};

		Ok(session)
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let control = self.control.run();
		let contribute = self.contribute.run();
		let distribute = self.distribute.run();

		tokio::try_join!(control, contribute, distribute)?;

		Ok(())
	}
}
