use clap::{Parser, Subcommand};

use crate::{client::ClientConfig, server::ServerConfig};

#[derive(Parser, Clone)]
pub struct Config {
	/// Listen for UDP packets on the given address.
	#[arg(long, default_value = "[::]:0")]
	pub bind: std::net::SocketAddr,

	/// Log configuration.
	#[command(flatten)]
	pub log: moq_native::log::Args,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Args,

	/// If we're publishing or subscribing.
	#[command(subcommand)]
	pub command: Command,

	/// The name of the room to join.
	#[arg(long)]
	pub room: String,

	/// The name of the broadcast within the room.
	#[arg(long)]
	pub name: String,
}

#[derive(Subcommand, Clone)]
pub enum Command {
	/// Host a server, accepting connections from clients.
	Serve(ServerConfig),

	/// Publish a video stream to the provided URL.
	Publish(ClientConfig),
}
