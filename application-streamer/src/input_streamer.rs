use tokio::io::{AsyncRead};
use std::net;
use std::str::FromStr;
use anyhow::Context;
use moq_transfork::Session;
use url::Url;
use moq_karp::{cmaf, BroadcastProducer};
use moq_native::quic;

pub struct MoQInputStreamer<T: AsyncRead + Unpin> {
    log: moq_native::log::Args,
    tls: moq_native::tls::Args,
    bind: net::SocketAddr,
    port: u16,
    input: T,
}

impl<T: AsyncRead + Unpin> MoQInputStreamer<T> {
    pub fn new(port: u16, input: T) -> Self {
        Self {
            log: moq_native::log::Args::default(),
            tls: moq_native::tls::Args::default(),
            bind: net::SocketAddr::from_str("[::]:0").unwrap(),
            port,
            input,
        }
    }

    pub async fn start(&mut self) {
        self.log.init();
        self.publish(format!("http://host.docker.internal:{}/", self.port)).await.unwrap();
    }

    #[tracing::instrument(skip_all, fields(?url))]
    async fn publish(&mut self, url: String) -> anyhow::Result<()> {
        let (session, path) = self.connect(&url).await?;
        let broadcast = BroadcastProducer::new(session.clone(), path)?;

        let mut import = cmaf::Import::new(broadcast);
        import.init_from(&mut self.input).await.context("failed to initialize")?;

        tracing::info!("publishing");

        tokio::select! {
            res = import.read_from(&mut self.input) => Ok(res?),
            res = session.closed() => Err(res.into()),
        }
    }

    async fn connect(&self, url: &str) -> anyhow::Result<(Session, String)> {
        let tls = self.tls.load()?;
        let quic = quic::Endpoint::new(quic::Config { bind: self.bind, tls })?;

        tracing::info!(?url, "connecting");

        let url = Url::parse(url).context("invalid URL")?;
        let path = url.path().to_string();

        let session = quic.client.connect(url).await?;
        let session = Session::connect(session).await?;

        Ok((session, path))
    }
}