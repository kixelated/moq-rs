use anyhow::Context;
use http;
use moq_transport::{Message, Object};
use std::net;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

pub struct SessionRunner {
	moq_transport_session: moq_transport_quinn::Session,
	outgoing_ctl_sender: mpsc::Sender<moq_transport::Message>,
	outgoing_ctl_receiver: mpsc::Receiver<moq_transport::Message>,
	outgoing_obj_sender: mpsc::Sender<moq_transport::Object>,
	outgoing_obj_receiver: mpsc::Receiver<moq_transport::Object>,
	incoming_ctl_sender: broadcast::Sender<moq_transport::Message>,
	incoming_ctl_receiver: broadcast::Receiver<moq_transport::Message>,
	incoming_obj_sender: broadcast::Sender<moq_transport::Object>,
	incoming_obj_receiver: broadcast::Receiver<moq_transport::Object>,
}

pub struct Config {
	pub addr: net::SocketAddr,
	pub uri: http::uri::Uri,
}

impl SessionRunner {
	pub async fn new(config: Config) -> anyhow::Result<Self> {
		// Ugh, just let me use my native root certs already
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
		let moq_transport_session = moq_transport_quinn::connect(webtransport_session, moq_transport::Role::Both)
			.await
			.context("failed to create MoQ Transport session")?;

		// outgoing ctl msgs
		let (outgoing_ctl_sender, outgoing_ctl_receiver) = mpsc::channel(5);
		// outgoing obs
		let (outgoing_obj_sender, outgoing_obj_receiver) = mpsc::channel(5);
		// incoming ctl msg
		let (incoming_ctl_sender, incoming_ctl_receiver) = broadcast::channel(5);
		// incoming objs
		let (incoming_obj_sender, incoming_obj_receiver) = broadcast::channel(5);

		Ok(SessionRunner {
			moq_transport_session,
			outgoing_ctl_sender,
			outgoing_ctl_receiver,
			outgoing_obj_sender,
			outgoing_obj_receiver,
			incoming_ctl_sender,
			incoming_ctl_receiver,
			incoming_obj_sender,
			incoming_obj_receiver,
		})
	}
	pub async fn get_outgoing_senders(
		&self,
	) -> (
		mpsc::Sender<moq_transport::Message>,
		mpsc::Sender<moq_transport::Object>,
	) {
		(self.outgoing_ctl_sender.clone(), self.outgoing_obj_sender.clone())
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
		dbg!("session_runner.run()");

		let mut join_set: JoinSet<anyhow::Result<()>> = tokio::task::JoinSet::new();

		// Send outgoing control messages
		join_set.spawn(async move {
			loop {
				dbg!();
				let msg = self
					.outgoing_ctl_receiver
					.recv()
					.await
					.ok_or(anyhow::anyhow!("error receiving outbound control message"))?;
				dbg!(&msg);
				self.moq_transport_session.send_control.send(msg).await?;
			}
		});

		// Route incoming Control messages
		join_set.spawn(async move {
			loop {
				dbg!();
				let msg = self.moq_transport_session.recv_control.recv().await?;
				dbg!(&msg);
				self.incoming_ctl_sender.send(msg)?;
			}
		});
			}
		});

		while let Some(res) = join_set.join_next().await {
			dbg!(&res);
		}

		Ok(())
	}
}
