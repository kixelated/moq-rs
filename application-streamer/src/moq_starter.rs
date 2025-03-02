use std::net;
use anyhow::Context;
use moq_transfork::Session;
use url::Url;
use moq_karp::{cmaf, BroadcastProducer};
use moq_native::quic;

pub struct MoqStarter {
    log: moq_native::log::Args,
    tls: moq_native::tls::Args,
    bind: net::SocketAddr,
}

impl MoqStarter {
    pub fn new(bind: net::SocketAddr) -> Self {
        Self {
            log: moq_native::log::Args::default(),
            tls: moq_native::tls::Args::default(),
            bind,
        }
    }

    pub async fn start(&self) {
        self.log.init();
        self.publish("http://localhost:8080/demo/bbb".to_string()).await.unwrap();
    }

    #[tracing::instrument(skip_all, fields(?url))]
    async fn publish(&self, url: String) -> anyhow::Result<()> {
        let (session, path) = self.connect(&url).await?;
        let broadcast = BroadcastProducer::new(session.clone(), path)?;
        let mut input = tokio::io::stdin();

        let mut import = cmaf::Import::new(broadcast);
        import.init_from(&mut input).await.context("failed to initialize")?;

        tracing::info!("publishing");

        tokio::select! {
            res = import.read_from(&mut input) => Ok(res?),
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