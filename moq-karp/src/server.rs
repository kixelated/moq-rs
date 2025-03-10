use std::net::SocketAddr;
use anyhow::Context;
use bytes::BytesMut;
use url::Url;
use moq_async::Lock;
use moq_native::quic;
use moq_native::quic::Server;
use moq_transfork::{web_transport};
use crate::{BroadcastProducer};
use crate::cmaf::Import;
use crate::fingerprint::FingerprintServer;
use tokio::io::AsyncRead;


pub struct BroadcastServer<T: AsyncRead + Unpin> {
    bind: SocketAddr,
    tls: moq_native::tls::Args,
    url: String,
    input: T,
}

impl<T: AsyncRead + Unpin> BroadcastServer<T> {
    pub fn new(bind: SocketAddr, tls: moq_native::tls::Args, url: String, input: T) -> Self {
        Self { bind, tls, url, input }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.bind = tokio::net::lookup_host(self.bind)
            .await
            .context("invalid bind address")?
            .next()
            .context("invalid bind address")?;

        let tls = self.tls.load()?;
        if tls.server.is_none() {
            anyhow::bail!("missing TLS certificates");
        }

        let quic = quic::Endpoint::new(quic::Config { bind: self.bind, tls: tls.clone() })?;
        let server = quic.server.context("missing TLS certificate")?;

        // Create a web server to serve the fingerprint.
        let web = FingerprintServer::new(self.bind, tls);
        tokio::spawn(async move {
            web.run().await.expect("failed to run web server");
        });

        // Create the broadcast
        let url = Url::parse(&self.url).context("invalid URL")?;
        let path = url.path().to_string();

        let broadcast = BroadcastProducer::new(path)?;

        let mut import = Import::new(broadcast);
        import.init_from(&mut self.input).await.context("failed to initialize cmaf from input")?;

        let lock = Lock::new(import);
        let import = lock.clone();

        self.accept(server, lock.clone())?;

        // TODO: figure out a way to not lock import while reading or accepting, as it will block the other operation.
        let mut buffer = BytesMut::new();
        let mut reading = true;
        while reading {
            let mut import = import.lock();
            reading = import.read_from_once(&mut self.input, &mut buffer).await?;
        }

        Ok(())
    }

    fn accept(&mut self, mut server: Server, import: Lock<Import>) -> anyhow::Result<()> {
        tracing::info!(addr = %self.bind, "listening");

        let mut conn_id = 0;

        tokio::spawn(async move {
            while let Some(conn) = server.accept().await {
                // Create a new connection
                let session: web_transport::Session = conn.into();
                let transfork_session = moq_transfork::Session::accept(session).await.expect("failed to accept session");

                conn_id += 1;

                let mut import = import.lock();
                import.broadcast.add_session(transfork_session).expect("failed to add session");

                tracing::info!(id = conn_id.clone(), "accepted");
            }
        });

        Ok(())
    }
}