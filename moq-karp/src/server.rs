use std::net::SocketAddr;
use anyhow::Context;
use url::Url;
use moq_native::quic;
use moq_native::quic::Server;
use moq_transfork::web_transport;
use crate::{cmaf, BroadcastProducer};
use crate::fingerprint::FingerprintServer;

pub struct BroadcastServer {
    bind: SocketAddr,
    tls: moq_native::tls::Args,
    import: Option<cmaf::Import>
}

impl BroadcastServer {
    pub fn new(bind: SocketAddr, tls: moq_native::tls::Args) -> Self {
        Self { bind, tls, import: None }
    }

    pub async fn run(&mut self, url: String) -> anyhow::Result<()> {
        let bind = tokio::net::lookup_host(self.bind)
            .await
            .context("invalid bind address")?
            .next()
            .context("invalid bind address")?;

        let tls = self.tls.load()?;
        if tls.server.is_none() {
            anyhow::bail!("missing TLS certificates");
        }

        let quic = quic::Endpoint::new(quic::Config { bind, tls: tls.clone() })?;
        let server = quic.server.context("missing TLS certificate")?;

        // Create a web server to serve the fingerprint.
        let web = FingerprintServer::new(bind, tls);
        tokio::spawn(async move {
            web.run().await.expect("failed to run web server");
        });

        self.accept(bind, server, url).await
    }

    async fn accept(&mut self, bind: SocketAddr, mut server: Server, url: String) -> anyhow::Result<()> {
        tracing::info!(addr = %bind, "listening");

        let mut conn_id = 0;

        while let Some(conn) = server.accept().await {
            // Create a new connection
            let session: web_transport::Session = conn.into();
            let transfork_session = moq_transfork::Session::accept(session).await.expect("failed to accept session");

            conn_id += 1;

            tracing::info!(id = conn_id, "accepted");

            match &mut self.import {
                // If it's the first connection, set up the broadcast
                None => {
                    let url = Url::parse(&url).context("invalid URL")?;
                    let path = url.path().to_string();

                    let broadcast = BroadcastProducer::new(transfork_session, path)?;
                    let mut input = tokio::io::stdin();

                    let mut import = cmaf::Import::new(broadcast);
                    import.init_from(&mut input).await.context("failed to initialize cmaf from input")?;
                    self.import = Some(import);

                    tracing::info!("publishing");
                }
                // Otherwise, add the session to the existing broadcast
                Some(import) => {
                    tracing::info!("adding session to existing broadcast");
                    import.add_listener(transfork_session)?;
                }
            }
        }

        Ok(())
    }
}