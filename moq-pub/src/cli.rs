use clap::{Parser, ValueEnum};
use std::net;

#[derive(Parser, Clone)]
#[command(arg_required_else_help(true))]
pub struct Config {
	#[arg(long, hide_short_help = true, default_value = "[::]:0")]
	pub bind_address: net::SocketAddr,

	#[arg(short, long, default_value = "https://localhost:4443")]
	pub uri: http::uri::Uri,

	#[arg(short, long, required = true, value_parser=input_parser)]
	input: InputValues,

	#[arg(long, hide_short_help = true, default_value = "24")]
	pub catalog_fps: u8,

	#[arg(long, hide_short_help = true, default_value = "1500000")]
	pub catalog_bit_rate: u32,
}

fn input_parser(s: &str) -> Result<InputValues, String> {
	if s == "-" {
		return Ok(InputValues::Stdin);
	}
	Err("The only currently supported input value is: '-' (stdin)".to_string())
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum InputValues {
	Stdin,
}
