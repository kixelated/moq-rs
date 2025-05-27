use clap::{Parser, Subcommand, ValueEnum};
use std::{fs, io, path::PathBuf};

#[derive(Debug, Clone, Copy, Parser, ValueEnum, Eq, PartialEq)]
#[clap(rename_all = "kebab-case")]
enum KeyFormat {
	/// DER-encoded keys.
	Der,
	// TODO somebody please add support for these formats.
	// Pem,
	// Jwk,
}

#[derive(Debug, Parser)]
#[command(name = "moq-token")]
#[command(about = "Generate, sign, and verify tokens for moq-relay", long_about = None)]
struct Cli {
	/// The algorithm to use for the token.
	#[arg(long, default_value = "HS256")]
	algorithm: moq_token::Algorithm,

	/// The format of the key.
	#[arg(long, value_enum, default_value_t = KeyFormat::Der)]
	format: KeyFormat,

	/// The command to execute.
	#[command(subcommand)]
	command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
	/// Generate a key (pair) for the given algorithm
	Generate {
		/// The path to write the private key for signing.
		#[arg(long)]
		sign: PathBuf,

		/// The path to write the public key for verification.
		///
		/// NOTE: When HS* algorithms are used, the public key is the same as the private key.
		#[arg(long)]
		verify: PathBuf,
	},

	/// Sign a token to stdout.
	Sign {
		/// The path to the private key used for signing.
		#[arg(long)]
		key: PathBuf,

		/// The payload to sign.
		#[command(flatten)]
		payload: moq_token::Payload,
	},

	/// Verify a token from stdin.
	Verify {
		/// The path to the public key used for verification.
		#[arg(long)]
		key: PathBuf,
	},
}

fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();

	match cli.command {
		Commands::Generate { sign, verify } => {
			let (encode, decode) = moq_token::generate(cli.algorithm);
			fs::write(sign, encode)?;
			fs::write(verify, decode)?;
		}

		Commands::Sign { payload, key } => {
			let key = fs::read(key)?;
			let encode = moq_token::Encoder::new(cli.algorithm, &key);
			let token = encode.sign(&payload);
			print!("{token}");
		}

		Commands::Verify { key } => {
			let key = fs::read(key)?;
			let decoder = moq_token::Decoder::new(cli.algorithm, &key);
			let token = io::read_to_string(io::stdin())?;

			let payload = decoder.decode(&token)?;
			println!("{:#?}", payload);
		}
	}

	Ok(())
}
