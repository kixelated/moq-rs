use warp::server::Server;

use clap::Parser;

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
}

fn main() {
    let args = Cli::parse();

    let server_config = warp::server::Config{
        addr: args.addr,
        cert: args.cert,
        key: args.key,
    };

    let mut server = Server::new(server_config).unwrap();
    loop {
        server.poll().unwrap()
    }
}