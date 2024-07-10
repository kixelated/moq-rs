use std::net;

use anyhow::Context;

use futures::{stream::FuturesUnordered, StreamExt};
use moq_native::quic;
use moq_transfork::{model, Broadcast, RouterReader, RouterWriter};
use url::Url;

use crate::{Origins, Session};

pub struct RelayConfig {
	/// Listen on this address
	pub bind: net::SocketAddr,

	/// The TLS configuration.
	pub tls: moq_native::tls::Config,

	/// Forward announcements to the (optional) URL.
	/// If not provided, then we can't discover other origins.
	pub announce: Option<Url>,

	/// Our hostname which we advertise to other origins.
	/// We use QUIC, so the certificate must be valid for this address.
	/// If not provided, we don't advertise our origin.
	pub host: Option<String>,
}

pub struct Relay {
	config: RelayConfig,
	outgoing: Origins,
	incoming: (RouterWriter<Broadcast>, RouterReader<Broadcast>),
}

impl Relay {
	// Create a QUIC endpoint that can be used for both clients and servers.
	pub fn new(config: RelayConfig) -> Self {
		Self {
			config,
			outgoing: Origins::default(),
			incoming: model::Router::produce(),
		}
	}

	pub async fn run(mut self) -> anyhow::Result<()> {
		let mut tasks = FuturesUnordered::new();

		let quic = quic::Endpoint::new(quic::Config {
			bind: self.config.bind,
			tls: self.config.tls,
		})?;

		/*
		let root = if let Some(url) = self.config.announce {
			let conn = quic
				.client
				.connect(&url)
				.await
				.context("failed to connect to announce server")?;

			let (session, publisher, subscriber) = moq_transfork::Session::connect(conn)
				.await
				.context("failed to establish announce session")?;

			tasks.push(session.run().boxed());
			Some((publisher, subscriber))
		} else {
			None
		};
		*/

		// let remotes = Remotes::new();

		let mut server = quic.server.context("missing TLS certificate")?;

		tracing::info!(bind = %self.config.bind, "listening");

		loop {
			tokio::select! {
				Some(conn) = server.accept() => {
					let session = Session::new(conn, self.outgoing.clone(), self.incoming.1.clone());
					tasks.push(session.run());
				},
				_ = self.outgoing.serve(&mut self.incoming.0) => anyhow::bail!("router serve finished"),
				_ = tasks.next(), if !tasks.is_empty() => {},
				else => return Ok(()),
			}
		}
	}
}
