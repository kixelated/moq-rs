use std::io::Write;
use tokio::io::{AsyncRead, AsyncReadExt};

pub struct InputWriter {
    file: std::fs::File,
}

impl InputWriter {
    pub fn new(file_location: &str) -> Self {
        let file = std::fs::File::create(file_location).expect("failed to create file");
        Self { file }
    }

    pub async fn write<T: AsyncRead + Unpin>(&mut self, mut input: T) -> anyhow::Result<()> {
        let mut file = std::fs::OpenOptions::new().write(true).open("file.txt")?;

        let mut buffer = [0; 1024];
        loop {
            let n = input.read(&mut buffer).await?;
            if n != 0 {
                file.write_all(&buffer[..n])?;
            }
        }

        Ok(())
    }
}