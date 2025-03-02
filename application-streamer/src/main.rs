use std::net;
use std::net::IpAddr;
use std::str::FromStr;

mod moq_starter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let moq_starter = moq_starter::MoqStarter::new(
		net::SocketAddr::new(IpAddr::from_str("0.0.0.0")?, 4443)
	);

	moq_starter.start().await;

	Ok(())
}
