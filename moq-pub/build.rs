include!("src/cli.rs");

use clap::CommandFactory;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let out_dir = std::path::PathBuf::from(
		std::env::var_os("OUT_DIR").ok_or(std::io::Error::new(std::io::ErrorKind::NotFound, "OUT_DIR not found"))?,
	);
	let cmd = Config::command();
	let man = clap_mangen::Man::new(cmd);
	let mut buffer: Vec<u8> = Default::default();
	man.render(&mut buffer)?;
	std::fs::write(out_dir.join("moq-pub.1"), buffer)?;
	Ok(())
}
