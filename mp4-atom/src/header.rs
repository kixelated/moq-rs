use std::io::{Cursor, Read};

use crate::*;

/// A atom header, which contains the atom's kind and size.
#[derive(Debug, Clone, Copy)]
pub struct Header {
    /// The name of the atom, always 4 bytes.
    pub kind: FourCC,

    /// The size of the atom, **excluding** the header.
    /// This is optional when the atom extends to the end of the file.
    pub size: Option<usize>,
}

impl Encode for Header {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        match self.size.map(|size| size + 8) {
            Some(size) if size > u32::MAX as usize => {
                1u32.encode(buf)?;
                self.kind.encode(buf)?;

                // Have to include the size of this extra field
                ((size + 8) as u64).encode(buf)
            }
            Some(size) => {
                (size as u32).encode(buf)?;
                self.kind.encode(buf)
            }
            None => {
                0u32.encode(buf)?;
                self.kind.encode(buf)
            }
        }
    }
}

impl Decode for Header {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let size = u32::decode(buf)?;
        let kind = FourCC::decode(buf)?;

        let size = match size {
            0 => None,
            1 => {
                // Read another 8 bytes
                let size = u64::decode(buf)?;
                Some(size.checked_sub(16).ok_or(Error::InvalidSize)? as usize)
            }
            _ => Some(size.checked_sub(8).ok_or(Error::InvalidSize)? as usize),
        };

        Ok(Self { kind, size })
    }
}

impl DecodeMaybe for Header {
    fn decode_maybe<B: Buf>(buf: &mut B) -> Result<Option<Self>> {
        if buf.remaining() < 8 {
            return Ok(None);
        }

        let size = u32::from_be_bytes(buf.slice(4).try_into().unwrap());
        if size == 1 && buf.remaining() < 16 {
            return Ok(None);
        }

        Ok(Some(Self::decode(buf)?))
    }
}

impl ReadFrom for Header {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        <Option<Header> as ReadFrom>::read_from(r)?.ok_or(Error::UnexpectedEof)
    }
}

impl ReadFrom for Option<Header> {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let mut buf = [0u8; 8];
        let n = r.read(&mut buf)?;
        if n == 0 {
            return Ok(None);
        }

        r.read_exact(&mut buf[n..])?;

        let size = u32::from_be_bytes(buf[0..4].try_into().unwrap());
        let kind = u32::from_be_bytes(buf[4..8].try_into().unwrap()).into();

        let size = match size {
            0 => None,
            1 => {
                // Read another 8 bytes
                r.read_exact(&mut buf)?;
                let size = u64::from_be_bytes(buf);
                let size = size.checked_sub(16).ok_or(Error::InvalidSize)?;

                Some(size as usize)
            }
            _ => Some(size.checked_sub(8).ok_or(Error::InvalidSize)? as usize),
        };

        Ok(Some(Header { kind, size }))
    }
}

// Utility methods
impl Header {
    pub(crate) fn read_body<R: Read>(&self, r: &mut R) -> Result<Cursor<Vec<u8>>> {
        // TODO This allocates on the heap.
        // Ideally, we should use ReadFrom instead of Decode to avoid this.

        // Don't use `with_capacity` on an untrusted size
        // We allocate at most 4096 bytes upfront and grow as needed
        let cap = self.size.unwrap_or(0).min(4096);
        let mut buf = Vec::with_capacity(cap);

        match self.size {
            Some(size) => {
                let n = std::io::copy(&mut r.take(size as _), &mut buf)? as _;
                if size != n {
                    return Err(Error::OutOfBounds);
                }
            }
            None => {
                std::io::copy(r, &mut buf)?;
            }
        };

        Ok(Cursor::new(buf))
    }

    #[cfg(feature = "tokio")]
    pub(crate) async fn read_body_tokio<R: ::tokio::io::AsyncRead + Unpin>(
        &self,
        r: &mut R,
    ) -> Result<Cursor<Vec<u8>>> {
        use ::tokio::io::AsyncReadExt;

        // TODO This allocates on the heap.
        // Ideally, we should use ReadFrom instead of Decode to avoid this.

        // Don't use `with_capacity` on an untrusted size
        // We allocate at most 4096 bytes upfront and grow as needed
        let cap = self.size.unwrap_or(0).min(4096);
        let mut buf = Vec::with_capacity(cap);

        match self.size {
            Some(size) => {
                let n = ::tokio::io::copy(&mut r.take(size as _), &mut buf).await? as _;
                if size != n {
                    return Err(Error::OutOfBounds);
                }
            }
            None => {
                ::tokio::io::copy(r, &mut buf).await?;
            }
        };

        Ok(Cursor::new(buf))
    }
}
