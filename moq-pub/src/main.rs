use anyhow::Context;
use clap::Parser;

mod client;
use client::*;

#[derive(Parser, Clone)]
struct Cli {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();

	let args = Cli::parse();

	println!("Hello, world!");
	Ok(())
}
