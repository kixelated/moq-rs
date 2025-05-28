use anyhow::Context;
use clap::{Parser, Subcommand};
use std::{io, path::PathBuf};

#[derive(Debug, Parser)]
#[command(name = "moq-token")]
#[command(about = "Generate, sign, and verify tokens for moq-relay", long_about = None)]
struct Cli {
	/// The path for the key.
	#[arg(long)]
	key: PathBuf,

	/// The command to execute.
	#[command(subcommand)]
	command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
	/// Generate a key (pair) for the given algorithm.
	///
	/// The key is output to the provided -key path.
	Generate {
		/// The algorithm to use.
		#[arg(long, default_value = "HS256")]
		algorithm: moq_token::Algorithm,

		/// An optional key ID, useful for rotating keys.
		#[arg(long)]
		id: Option<String>,
	},

	/// Sign a token to stdout, reading the key from stdin.
	// NOTE: This is a superset of payload because of limitations in clap.
	Sign {
		/// The base path. Any paths are relative to this path.
		#[arg(long, default_value = "")]
		path: String,

		/// If specified, the user can publish any broadcasts matching a prefix.
		#[arg(long)]
		publish: Option<String>,

		/// If specified, the user will publish this path.
		/// No announcement is needed, and the broadcast is considered active while the connection is active.
		/// This is useful to avoid an RTT and informs all other clients that this user is connected.
		#[arg(long)]
		publish_force: Option<String>,

		/// If specified, the user can subscribe to any broadcasts matching a prefix.
		#[arg(long)]
		subscribe: Option<String>,

		/// The expiration time of the token as a unix timestamp.
		#[arg(long, value_parser = parse_unix_timestamp)]
		expires: Option<std::time::SystemTime>,

		/// The issued time of the token as a unix timestamp.
		#[arg(long, value_parser = parse_unix_timestamp)]
		issued: Option<std::time::SystemTime>,
	},

	/// Verify a token from stdin, writing the payload to stdout.
	Verify,
}

fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();

	match cli.command {
		Commands::Generate { algorithm, id } => {
			let key = moq_token::Key::generate(algorithm, id);
			key.to_file(cli.key)?;
		}

		Commands::Sign {
			path,
			publish,
			publish_force,
			subscribe,
			expires,
			issued,
		} => {
			let key = moq_token::Key::from_file(cli.key)?;

			let payload = moq_token::Payload {
				path,
				publish,
				publish_force,
				subscribe,
				expires,
				issued,
			};
			let token = key.sign(&payload)?;
			println!("{token}");
		}

		Commands::Verify => {
			let key = moq_token::Key::from_file(cli.key)?;
			let token = io::read_to_string(io::stdin())?;

			let payload = key.verify(&token)?;
			println!("{:#?}", payload);
		}
	}

	Ok(())
}

// A simpler parser for clap
fn parse_unix_timestamp(s: &str) -> anyhow::Result<std::time::SystemTime> {
	let timestamp = s.parse::<i64>().context("expected unix timestamp")?;
	let timestamp = timestamp.try_into().context("timestamp out of range")?;
	Ok(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp))
}
