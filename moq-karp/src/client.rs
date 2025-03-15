use std::net::SocketAddr;
use anyhow::Context;
use url::Url;
use moq_native::quic;
use crate::{BroadcastProducer};
use crate::cmaf::Import;
use crate::moq_transfork::Session;
use tokio::io::AsyncRead;

pub struct BroadcastClient<T: AsyncRead + Unpin> {
    bind: SocketAddr,
    tls: moq_native::tls::Args,
    url: String,
    input: T,
}
impl <T: AsyncRead + Unpin> BroadcastClient<T> {
    pub fn new(bind: SocketAddr, tls: moq_native::tls::Args, url: String, input: T) -> Self {
        Self { bind, tls, url, input }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let session = self.connect().await?;
        let url = Url::parse(&self.url).context("invalid URL")?;
        let path = url.path().to_string();

        let mut broadcast = BroadcastProducer::new(path)?;

        let mut import = Import::new();
        import.init_from(&mut self.input, &mut broadcast).await.context("failed to initialize cmaf from input")?;

        broadcast.add_session(session.clone())?;

        tracing::info!("publishing");

        tokio::select! {
            res = import.read_from(&mut self.input) => Ok(res?),
            res = session.closed() => Err(res.into()),
        }
    }

    async fn connect(&self) -> anyhow::Result<Session> {
        let tls = self.tls.load()?;
        let quic = quic::Endpoint::new(quic::Config { bind: self.bind, tls })?;

        tracing::info!(?self.url, "connecting");

        let url = Url::parse(&self.url).context("invalid URL")?;

        let session = quic.client.connect(url).await?;
        let session = Session::connect(session).await?;

        Ok(session)
    }
}