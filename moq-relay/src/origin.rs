use std::sync::{Arc, Mutex};

use moq_dir::{ListingReader, ListingWriter};
use moq_transfork::{Announce, Broadcast, Publisher};

#[derive(Clone)]
pub struct Origin {
	listing: Arc<Mutex<(ListingWriter, ListingReader)>>,
	announce: Announce,
}

impl Origin {
	pub fn new(root: Publisher, host: &str) -> Self {
		let (writer, reader) = Broadcast {
			name: format!(".origin.{}", host),
		}
		.produce();

		let listings = ListingWriter::new(writer.create("broadcasts").build().unwrap());
	}

	pub async fn run(mut self) -> anyhow::Result<()> {
		let announce = root.announce(reader);
		Ok(())
	}
}
