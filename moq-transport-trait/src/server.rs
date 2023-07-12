
use anyhow::Context;
use moq_generic_transport::{Connection, BidiStream, SendStream, SendStreamUnframed, RecvStream};
use moq_transport::{Message, SetupClient, SetupServer};

use crate::SharedConnection;

use super::{Control, Objects};
// pub struct Server<C: Connection> {
// 	// The Webtransport/QUIC server, with an already established session/connection.
// 	endpoint: Box<C>,
// }

// impl<C: Connection> Server<C> {
// 	pub fn new(endpoint: Box<C>) -> Self {
// 		let handshake = JoinSet::new();
// 		Self { endpoint }
// 	}

// 	// Accept the next WebTransport session.
// 	pub async fn accept(&mut self) -> anyhow::Result<Connect> {
// 		loop {
// 			tokio::select!(
// 				// Accept the connection and start the WebTransport handshake.
// 				conn = self.endpoint.accept() => {
// 					let conn = conn.context("failed to accept connection")?;
// 					self.handshake.spawn(async move {
// 						Connecting::new(conn).accept().await
// 					});
// 				},
// 				// Return any mostly finished WebTransport handshakes.
// 				res = self.handshake.join_next(), if !self.handshake.is_empty() => {
// 					let res = res.expect("no tasks").expect("task aborted");
// 					match res {
// 						Ok(session) => return Ok(session),
// 						Err(err) => log::warn!("failed to accept session: {:?}", err),
// 					}
// 				},
// 			)
// 		}
// 	}
// }


pub struct Session<S: SendStream + SendStreamUnframed, R: RecvStream + Send, B: BidiStream<SendStream = S, RecvStream = R>, C: Connection<SendStream = S, RecvStream = R, BidiStream = B> + Send> {
	pub control: Control<S, C::BidiStream>,
	pub objects: Objects<C>,
}

impl<S: SendStream + SendStreamUnframed, B: BidiStream<SendStream = S, RecvStream = R>, R: RecvStream + Send + 'static, C: Connection<SendStream = S, RecvStream = R, BidiStream = B> + Send> Session<S, R, B, C> {

	pub async fn accept(control_stream: Box<C::BidiStream>, connection: Box<C>) -> anyhow::Result<AcceptSetup<S, R, B, C>> {
		let mut control = Control::new(control_stream);
		let objects = Objects::new(std::sync::Arc::new(std::sync::Mutex::new(connection)));

		let setup_client = match control.recv().await.context("failed to read SETUP")? {
			Message::SetupClient(setup) => setup,
			_ => anyhow::bail!("expected CLIENT SETUP"),
		};
		Ok(AcceptSetup { setup_client, control, objects })

	}
	pub fn split(self) -> (Control<B::SendStream, C::BidiStream>, Objects<C>) {
		(self.control, self.objects)
	}
}


pub struct AcceptSetup<S: SendStream + SendStreamUnframed, R: RecvStream + Send, B: BidiStream<SendStream = S, RecvStream = R>, C: Connection<SendStream = S, RecvStream = R, BidiStream = B> + Send> {
	setup_client: SetupClient,
	control: Control<S, C::BidiStream>,
	objects: Objects<C>,
}

impl<S: SendStream + SendStreamUnframed, R: RecvStream + Send, B: BidiStream<SendStream = S, RecvStream = R>, C: Connection<SendStream = S, RecvStream = R, BidiStream = B> + Send> AcceptSetup<S, R, B, C> {
	// Return the setup message we received.
	pub fn setup(&self) -> &SetupClient {
		&self.setup_client
	}

	// Accept the session with our own setup message.
	pub async fn accept(mut self, setup_server: SetupServer) -> anyhow::Result<Session<S, R, B, C>> {
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