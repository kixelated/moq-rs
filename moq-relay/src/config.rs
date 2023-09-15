use std::{net, path};

use clap::Parser;

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser, Clone)]
pub struct Config {
	/// Listen on this address
	#[arg(long, default_value = "[::]:4443")]
	pub bind: net::SocketAddr,

	/// Use the certificate file at this path
	#[arg(long)]
	pub cert: path::PathBuf,

	/// Use the private key at this path
	#[arg(long)]
	pub key: path::PathBuf,

	/// Listen on HTTPS and serve /fingerprint, for self-signed certificates
	#[arg(long, action)]
	pub fingerprint: bool,
}
