use clap::Parser;
use std::net;

#[derive(Parser, Clone, Debug)]
pub struct Config {
	#[arg(long, hide_short_help = true, default_value = "[::]:0")]
	pub bind: net::SocketAddr,

	#[arg(long, default_value = "localhost:4443")]
	pub host: String,

	#[arg(long, hide_short_help = true, default_value = "24")]
	pub catalog_fps: u8,

	#[arg(long, hide_short_help = true, default_value = "1500000")]
	pub catalog_bit_rate: u32,

	#[arg(long, required = false, default_value = "")]
	pub name: String,
}
