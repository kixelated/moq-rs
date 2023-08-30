use anyhow::Context;
use log::debug;
use moq_transport::{object, Object};
use std::net;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

pub struct SessionRunner {
	moq_transport_session: moq_transport::Session<webtransport_quinn::Session>,
	outgoing_ctl_sender: mpsc::Sender<moq_transport::Message>,
	outgoing_ctl_receiver: mpsc::Receiver<moq_transport::Message>,
	incoming_ctl_sender: broadcast::Sender<moq_transport::Message>,
	incoming_obj_sender: broadcast::Sender<Object>,
}

pub struct Config {
	pub addr: net::SocketAddr,
	pub uri: http::uri::Uri,
}

impl SessionRunner {
	pub async fn new(config: Config) -> anyhow::Result<Self> {
		let mut roots = rustls::RootCertStore::empty();
		for cert in rustls_native_certs::load_native_certs().expect("could not load platform certs") {
			roots.add(&rustls::Certificate(cert.0)).unwrap();
		}

		let mut tls_config = rustls::ClientConfig::builder()
			.with_safe_defaults()
			.with_root_certificates(roots)
			.with_no_client_auth();

		tls_config.alpn_protocols = vec![webtransport_quinn::ALPN.to_vec()]; // this one is important

		let arc_tls_config = std::sync::Arc::new(tls_config);
		let quinn_client_config = quinn::ClientConfig::new(arc_tls_config);

		let mut endpoint = quinn::Endpoint::client(config.addr)?;
		endpoint.set_default_client_config(quinn_client_config);

		let webtransport_session = webtransport_quinn::connect(&endpoint, &config.uri)
			.await
			.context("failed to create WebTransport session")?;
		let moq_transport_session =
			moq_transport::Session::connect(webtransport_session, moq_transport::setup::Role::Both)
				.await
				.context("failed to create MoQ Transport session")?;

		// outgoing ctl msgs
		let (outgoing_ctl_sender, outgoing_ctl_receiver) = mpsc::channel(5);
		// incoming ctl msg
		let (incoming_ctl_sender, _incoming_ctl_receiver) = broadcast::channel(5);
		// incoming objs
		let (incoming_obj_sender, _incoming_obj_receiver) = broadcast::channel(5);

		Ok(SessionRunner {
			moq_transport_session,
			outgoing_ctl_sender,
			outgoing_ctl_receiver,
			incoming_ctl_sender,
			incoming_obj_sender,
		})
	}
	pub async fn get_outgoing_senders(&self) -> mpsc::Sender<moq_transport::Message> {
		self.outgoing_ctl_sender.clone()
	}
	pub async fn get_incoming_receivers(
		&self,
	) -> (
		broadcast::Receiver<moq_transport::Message>,
		broadcast::Receiver<moq_transport::Object>,
	) {
		(
			self.incoming_ctl_sender.subscribe(),
			self.incoming_obj_sender.subscribe(),
		)
	}
	pub async fn run(mut self) -> anyhow::Result<()> {
		debug!("session_runner.run()");

		let mut join_set: JoinSet<anyhow::Result<()>> = tokio::task::JoinSet::new();

		// Send outgoing control messages
		join_set.spawn(async move {
			loop {
				let msg = self
					.outgoing_ctl_receiver
					.recv()
					.await
					.ok_or(anyhow::anyhow!("error receiving outbound control message"))?;
				debug!("Sending outgoing MOQT Control Message: {:?}", &msg);
				self.moq_transport_session.send_control.send(msg).await?;
			}
		});

		// Route incoming Control messages
		join_set.spawn(async move {
			loop {
				let msg = self.moq_transport_session.recv_control.recv().await?;
				self.incoming_ctl_sender.send(msg)?;
			}
		});

		// Route incoming Objects headers
		// NOTE: Only sends the headers for incoming objects, not the associated streams
		// We don't currently expose any way to read incoming bytestreams because we don't expect any
		join_set.spawn(async move {
			loop {
				let receive_stream = self.moq_transport_session.recv_objects.recv().await?;

				self.incoming_obj_sender.send(receive_stream.0)?;
			}
		});

		while let Some(res) = join_set.join_next().await {
			debug!("SessionRunner task finished with result: {:?}", &res);
			let _ = res?; // if we finish, it'll be with an error, which we can return
		}

		Ok(())
	}

	pub async fn get_send_objects(&self) -> object::Sender<webtransport_quinn::Session> {
		self.moq_transport_session.send_objects.clone()
	}
}
