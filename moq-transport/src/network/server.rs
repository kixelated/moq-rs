
use anyhow::Context;
use webtransport_generic::{Connection, RecvStream};
use crate::{Message, SetupClient, SetupServer};


use super::{Control, Objects};
pub struct Session<C: Connection + Send> {
	pub control: Control<C::SendStream, C::RecvStream>,
	pub objects: Objects<C>,
}

impl<R: RecvStream + 'static, C: Connection<RecvStream = R> + Send> Session<C> {

	pub async fn accept(control_stream_send: Box<C::SendStream>, control_stream_recv: Box::<C::RecvStream>, connection: Box<C>) -> anyhow::Result<AcceptSetup<C>> {
		let mut control = Control::new(control_stream_send, control_stream_recv);
		let objects = Objects::new(std::sync::Arc::new(std::sync::Mutex::new(connection)));

		let setup_client = match control.recv().await.context("failed to read SETUP")? {
			Message::SetupClient(setup) => setup,
			_ => anyhow::bail!("expected CLIENT SETUP"),
		};
		Ok(AcceptSetup { setup_client, control, objects })

	}
	pub fn split(self) -> (Control<C::SendStream, C::RecvStream>, Objects<C>) {
		(self.control, self.objects)
	}
}


pub struct AcceptSetup<C: Connection + Send> {
	setup_client: SetupClient,
	control: Control<C::SendStream, C::RecvStream>,
	objects: Objects<C>,
}

impl<C: Connection + Send> AcceptSetup<C> {
	// Return the setup message we received.
	pub fn setup(&self) -> &SetupClient {
		&self.setup_client
	}

	// Accept the session with our own setup message.
	pub async fn accept(mut self, setup_server: SetupServer) -> anyhow::Result<Session<C>> {
		self.control.send(setup_server).await?;
		Ok(Session {
			control: self.control,
			objects: self.objects,
		})
	}

	pub async fn reject(self) -> anyhow::Result<()> {
		// TODO Close the QUIC connection with an error code.
		Ok(())
	}
}