use anyhow;

pub struct Publisher {}

impl Publisher {
	pub async fn new() -> anyhow::Result<Publisher> {
		Ok(Publisher {})
	}

	pub async fn run(self) -> anyhow::Result<()> {
		Ok(())
	}
}
