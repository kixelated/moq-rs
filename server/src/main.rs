use quiche::h3::webtransport;
use warp::transport;

use std::time;

use clap::Parser;
use env_logger;

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser)]
struct Cli {
    /// Listen on this address
    #[arg(short, long, default_value = "127.0.0.1:4443")]
    addr: String,

    /// Use the certificate file at this path
    #[arg(short, long, default_value = "../cert/localhost.crt")]
    cert: String,

    /// Use the private key at this path
    #[arg(short, long, default_value = "../cert/localhost.key")]
    key: String,

    /// Use the media file at this path
    #[arg(short, long, default_value = "../media/fragmented.mp4")]
    media: String,
}

#[derive(Default)]
struct Connection {
    webtransport: Option<webtransport::ServerSession>,
}

impl transport::App for Connection {
    fn poll(&mut self, conn: &mut quiche::Connection, session: &mut webtransport::ServerSession) -> anyhow::Result<()> {
        if !conn.is_established() {
            // Wait until the handshake finishes
            return Ok(())
        }

        if self.webtransport.is_none() {
            self.webtransport = Some(webtransport::ServerSession::with_transport(conn)?)
        }

        let webtransport = self.webtransport.as_mut().unwrap();

        Ok(())
    }

    fn timeout(&self) -> Option<time::Duration> {
        None
    }
}
fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Cli::parse();

    let server_config = transport::Config{
        addr: args.addr,
        cert: args.cert,
        key: args.key,
    };

    let mut server = transport::Server::<Connection>::new(server_config).unwrap();
    server.run()
}