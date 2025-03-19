use crate::{Error, Header, Result};

use super::*;

use tokio::io::{AsyncRead, AsyncReadExt};

impl AsyncReadFrom for Header {
    async fn read_from<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        <Option<Header> as AsyncReadFrom>::read_from(r)
            .await?
            .ok_or(Error::UnexpectedEof)
    }
}

impl AsyncReadFrom for Option<Header> {
    async fn read_from<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        let mut buf = [0u8; 8];
        let n = r.read(&mut buf).await?;
        if n == 0 {
            return Ok(None);
        }

        r.read_exact(&mut buf[n..]).await?;

        let size = u32::from_be_bytes(buf[0..4].try_into().unwrap());
        let kind = u32::from_be_bytes(buf[4..8].try_into().unwrap()).into();

        let size = match size {
            0 => None,
            1 => {
                // Read another 8 bytes
                r.read_exact(&mut buf).await?;
                let size = u64::from_be_bytes(buf);
                let size = size.checked_sub(16).ok_or(Error::InvalidSize)?;

                Some(size as usize)
            }
            _ => Some(size.checked_sub(8).ok_or(Error::InvalidSize)? as usize),
        };

        Ok(Some(Header { kind, size }))
    }
}
