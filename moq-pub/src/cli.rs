use clap::Parser;
use std::net;

#[derive(Parser, Clone, Debug)]
pub struct Config {
	/// Listen for UDP packets on the given address.
	#[arg(long, default_value = "[::]:0")]
	pub bind: net::SocketAddr,

	/// Advertise this frame rate in the catalog (informational)
	// TODO auto-detect this from the input when not provided
	#[arg(long, default_value = "24")]
	pub fps: u8,

	/// Advertise this bit rate in the catalog (informational)
	// TODO auto-detect this from the input when not provided
	#[arg(long, default_value = "1500000")]
	pub bitrate: u32,

	/// Connect to the given URI starting with moq://
	#[arg(value_parser = moq_uri)]
	pub uri: http::Uri,
}

fn moq_uri(s: &str) -> Result<http::Uri, String> {
	let uri = http::Uri::try_from(s).map_err(|e| e.to_string())?;

	// Make sure the scheme is moq
	if uri.scheme_str() != Some("moq") {
		return Err("uri scheme must be moq".to_string());
	}

	Ok(uri)
}
