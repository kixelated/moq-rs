use std::ffi;

use crate::*;

/// Decode a type from a buffer.
pub trait Decode: Sized {
    /// Decode the type from the buffer.
    fn decode<B: Buf>(buf: &mut B) -> Result<Self>;

    /// Helper: Decode exactly size bytes from the buffer.
    fn decode_exact<B: Buf>(buf: &mut B, size: usize) -> Result<Self> {
        if buf.remaining() < size {
            return Err(Error::OutOfBounds);
        }

        let mut inner = buf.slice(size);
        let res = Self::decode(&mut inner)?;

        if inner.has_remaining() {
            return Err(Error::ShortRead);
        }

        buf.advance(size);

        Ok(res)
    }
}

/// Decode a type from a buffer if we have enough data.
pub trait DecodeMaybe: Sized {
    fn decode_maybe<B: Buf>(buf: &mut B) -> Result<Option<Self>>;
}

/// Decode an atom using the provided header
pub trait DecodeAtom: Sized {
    fn decode_atom<B: Buf>(header: &Header, buf: &mut B) -> Result<Self>;
}

/// Encode a type to a buffer.
//
// Why not BufMut?
// Well it's because we need to write the size of each atom.
// If we use BufMut, we can't seek backwards so we have to calculate it upfront.
// If we use BufMut or Vec, then we can write 0 for the size, then write the atom, then go back and fix the size.
pub trait Encode {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()>;

    #[cfg(test)]
    fn assert_encode_decode(&self)
    where
        Self: std::fmt::Debug + PartialEq + Decode,
    {
        let mut buf = Vec::new();
        Self::encode(self, &mut buf).unwrap();

        let mut cursor = std::io::Cursor::new(&buf);
        let decoded = Self::decode(&mut cursor).unwrap();

        assert_eq!(self, &decoded, "different decoded result");
    }
}

impl Decode for u8 {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self::from_be_bytes(<[u8; 1]>::decode(buf)?))
    }
}

impl Decode for i8 {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self::from_be_bytes(<[u8; 1]>::decode(buf)?))
    }
}

impl Decode for u16 {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self::from_be_bytes(<[u8; 2]>::decode(buf)?))
    }
}

impl Decode for i16 {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self::from_be_bytes(<[u8; 2]>::decode(buf)?))
    }
}

impl Decode for u32 {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self::from_be_bytes(<[u8; 4]>::decode(buf)?))
    }
}

impl Decode for i32 {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self::from_be_bytes(<[u8; 4]>::decode(buf)?))
    }
}

impl Decode for u64 {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self::from_be_bytes(<[u8; 8]>::decode(buf)?))
    }
}

impl Decode for i64 {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self::from_be_bytes(<[u8; 8]>::decode(buf)?))
    }
}

impl<const N: usize> Decode for [u8; N] {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        if buf.remaining() < N {
            return Err(Error::OutOfBounds);
        }

        let mut v = [0u8; N];
        v.copy_from_slice(buf.slice(N));
        buf.advance(N);

        Ok(v)
    }
}

impl<T: Decode> Decode for Vec<T> {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let mut vec = Vec::new();
        while buf.has_remaining() {
            let item = T::decode(buf)?;
            vec.push(item);
        }

        Ok(vec)
    }
}

impl Decode for String {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let mut bytes = Vec::new();
        while buf.has_remaining() {
            let byte = u8::decode(buf)?;
            if byte == 0 {
                break;
            }

            bytes.push(byte);
        }

        let str = ffi::CString::new(bytes).map_err(|err| Error::InvalidString(err.to_string()))?;
        str.into_string()
            .map_err(|err| Error::InvalidString(err.to_string()))
    }
}

impl Encode for u8 {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.to_be_bytes().encode(buf)
    }
}

impl Encode for i8 {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.to_be_bytes().encode(buf)
    }
}

impl Encode for i16 {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.to_be_bytes().encode(buf)
    }
}

impl Encode for u16 {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.to_be_bytes().encode(buf)
    }
}

impl Encode for u32 {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.to_be_bytes().encode(buf)
    }
}

impl Encode for i32 {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.to_be_bytes().encode(buf)
    }
}

impl Encode for u64 {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.to_be_bytes().encode(buf)
    }
}

impl Encode for i64 {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.to_be_bytes().encode(buf)
    }
}

impl<const N: usize> Encode for [u8; N] {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        buf.append_slice(self);
        Ok(())
    }
}

impl<T: Encode> Encode for &[T] {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        for item in self.iter() {
            item.encode(buf)?;
        }

        Ok(())
    }
}

impl<T: Encode> Encode for Option<T> {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        match self {
            Some(v) => v.encode(buf),
            None => Ok(()),
        }
    }
}

impl Encode for &str {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.as_bytes().encode(buf)?;
        0u8.encode(buf)?;
        Ok(())
    }
}

impl<T: Encode> Encode for Vec<T> {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        for item in self.iter() {
            item.encode(buf)?;
        }

        Ok(())
    }
}
