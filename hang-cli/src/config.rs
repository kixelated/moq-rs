use clap::{Parser, Subcommand};
use std::net;

#[derive(Parser, Clone)]
pub struct Config {
	/// Listen for UDP packets on the given address.
	#[arg(long, default_value = "[::]:0")]
	pub bind: net::SocketAddr,

	/// Log configuration.
	#[command(flatten)]
	pub log: moq_native::log::Args,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Args,

	/// If we're publishing or subscribing.
	#[command(subcommand)]
	pub command: Command,
}

#[derive(Subcommand, Clone)]
pub enum Command {
	/// Host a server, accepting connections from clients.
	Serve {
		/// The path of the broadcast to serve.
		path: String,
	},

	/// Publish a video stream to the provided URL.
	Publish {
		/// The URL must start with `https://` or `http://`.
		///
		/// - If `http` is used, a HTTP fetch to "/fingerprint" is first made to get the TLS certificiate fingerprint (insecure).
		///   The URL is then upgraded to `https`.
		///
		/// - If `https` is used, then A WebTransport connection is made via QUIC to the provided host/port.
		///   The path is used to identify the broadcast, with the rest of the URL (ex. query/fragment) currently ignored.
		url: String,
	},
}
