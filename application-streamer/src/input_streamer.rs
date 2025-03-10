use tokio::io::{AsyncRead};
use std::net;
use std::str::FromStr;
use anyhow::Context;
use moq_transfork::Session;
use url::Url;
use moq_karp::{cmaf, BroadcastProducer, BroadcastServer};
use moq_native::quic;

pub struct MoQInputStreamer {
    log: moq_native::log::Args,
    tls: moq_native::tls::Args,
    bind: net::SocketAddr,
    port: u16,
}

impl MoQInputStreamer {
    pub fn new(port: u16) -> Self {
        let mut tls = moq_native::tls::Args::default();
        tls.self_sign.push(String::from("localhost:4443"));
        tls.disable_verify = true;

        let bind = net::SocketAddr::from_str(format!("[::]:{}", port).as_str()).unwrap();

        Self {
            log: moq_native::log::Args::default(),
            tls,
            bind,
            port,
        }
    }

    pub async fn stream<T: AsyncRead + Unpin>(&mut self, input: T) -> anyhow::Result<()> {
        self.log.init();

        let mut server = BroadcastServer::new(
            self.bind,
            self.tls.clone(),
            String::from(format!("http://localhost:{}/", self.port)),
            input
        );

        server.run().await
    }
}