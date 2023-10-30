use std::{net, path};
use url::Url;

use clap::Parser;

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser, Clone)]
pub struct Config {
	/// Listen on this address
	#[arg(long, default_value = "[::]:4443")]
	pub listen: net::SocketAddr,

	/// Use the certificates at this path, encoded as PEM.
	///
	/// You can use this option multiple times for multiple certificates.
	/// The first match for the provided SNI will be used, otherwise the last cert will be used.
	/// You also need to provide the private key multiple times via `key``.
	#[arg(long)]
	pub tls_cert: Vec<path::PathBuf>,

	/// Use the private key at this path, encoded as PEM.
	///
	/// There must be a key for every certificate provided via `cert`.
	#[arg(long)]
	pub tls_key: Vec<path::PathBuf>,

	/// Use the TLS root at this path, encoded as PEM.
	///
	/// This value can be provided multiple times for multiple roots.
	/// If this is empty, system roots will be used instead
	#[arg(long)]
	pub tls_root: Vec<path::PathBuf>,

	/// Danger: Disable TLS certificate verification.
	///
	/// Fine for local development and between relays, but should be used in caution in production.
	#[arg(long)]
	pub tls_disable_verify: bool,

	/// Optional: Use the moq-api via HTTP to store origin information.
	#[arg(long)]
	pub api: Option<Url>,

	/// Our internal address which we advertise to other origins.
	/// We use QUIC, so the certificate must be valid for this address.
	/// This needs to be prefixed with https:// to use WebTransport.
	/// This is only used when --api is set and only for publishing broadcasts.
	#[arg(long)]
	pub api_node: Option<Url>,

	/// Enable development mode.
	/// Currently, this only listens on HTTPS and serves /fingerprint, for self-signed certificates
	#[arg(long, action)]
	pub dev: bool,
}
