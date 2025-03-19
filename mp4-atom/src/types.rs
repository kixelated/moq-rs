use std::fmt;

use crate::*;

// Re-export this common types.
pub use num::rational::Ratio;

/// A four-character code used to identify atoms.
#[derive(Clone, Copy, PartialEq, Eq)]
// TODO serialize as a string
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FourCC([u8; 4]);

impl FourCC {
    // Helper function to create a FourCC from a string literal
    // ex. FourCC::new(b"abcd")
    pub const fn new(value: &[u8; 4]) -> Self {
        FourCC(*value)
    }
}

impl From<u32> for FourCC {
    fn from(value: u32) -> Self {
        FourCC(value.to_be_bytes())
    }
}

impl From<FourCC> for u32 {
    fn from(cc: FourCC) -> Self {
        u32::from_be_bytes(cc.0)
    }
}

impl From<[u8; 4]> for FourCC {
    fn from(value: [u8; 4]) -> Self {
        FourCC(value)
    }
}

impl From<FourCC> for [u8; 4] {
    fn from(cc: FourCC) -> Self {
        cc.0
    }
}

impl From<&[u8; 4]> for FourCC {
    fn from(value: &[u8; 4]) -> Self {
        FourCC(*value)
    }
}

impl fmt::Display for FourCC {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = String::from_utf8_lossy(&self.0);
        write!(f, "{}", s)
    }
}

impl fmt::Debug for FourCC {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = String::from_utf8_lossy(&self.0);
        write!(f, "{}", s)
    }
}

impl Encode for FourCC {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.0.encode(buf)
    }
}

impl Decode for FourCC {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(FourCC(<[u8; 4]>::decode(buf)?))
    }
}

impl AsRef<[u8; 4]> for FourCC {
    fn as_ref(&self) -> &[u8; 4] {
        &self.0
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
#[allow(non_camel_case_types)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct u24([u8; 3]);

impl Decode for u24 {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self(<[u8; 3]>::decode(buf)?))
    }
}

impl Encode for u24 {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.0.encode(buf)
    }
}

impl From<u24> for u32 {
    fn from(value: u24) -> Self {
        u32::from_be_bytes([0, value.0[0], value.0[1], value.0[2]])
    }
}

impl TryFrom<u32> for u24 {
    type Error = std::array::TryFromSliceError;

    fn try_from(value: u32) -> std::result::Result<Self, Self::Error> {
        Ok(Self(value.to_be_bytes()[1..].try_into()?))
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
#[allow(non_camel_case_types)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct u48([u8; 6]);

impl Decode for u48 {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self(<[u8; 6]>::decode(buf)?))
    }
}

impl Encode for u48 {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.0.encode(buf)
    }
}

impl TryFrom<u64> for u48 {
    type Error = std::array::TryFromSliceError;

    fn try_from(value: u64) -> std::result::Result<Self, Self::Error> {
        Ok(Self(value.to_be_bytes()[2..].try_into()?))
    }
}

impl From<u48> for u64 {
    fn from(value: u48) -> Self {
        u64::from_be_bytes([
            0, 0, value.0[0], value.0[1], value.0[2], value.0[3], value.0[4], value.0[5],
        ])
    }
}

impl From<[u8; 6]> for u48 {
    fn from(value: [u8; 6]) -> Self {
        u48(value)
    }
}

impl AsRef<[u8; 6]> for u48 {
    fn as_ref(&self) -> &[u8; 6] {
        &self.0
    }
}

// The top N bits are the integer part, the bottom N bits are the fractional part.
// Somebody who cares should implement some math stuff.
#[derive(Copy, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FixedPoint<T> {
    int: T,
    dec: T,
}

impl<T: Copy> FixedPoint<T> {
    pub fn new(int: T, dec: T) -> Self {
        Self { int, dec }
    }

    pub fn integer(&self) -> T {
        self.int
    }

    pub fn decimal(&self) -> T {
        self.dec
    }
}

impl<T: Decode> Decode for FixedPoint<T> {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            int: T::decode(buf)?,
            dec: T::decode(buf)?,
        })
    }
}

impl<T: Encode> Encode for FixedPoint<T> {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.int.encode(buf)?;
        self.dec.encode(buf)
    }
}

impl<T: num::Zero> From<T> for FixedPoint<T> {
    fn from(value: T) -> Self {
        Self {
            int: value,
            dec: T::zero(),
        }
    }
}

impl<T> fmt::Debug for FixedPoint<T>
where
    T: num::Zero + fmt::Debug + PartialEq + Copy,
    f64: From<T>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.dec.is_zero() {
            write!(f, "{:?}", self.int)
        } else {
            write!(f, "{:?}", f64::from(self.int) / f64::from(self.dec))
        }
    }
}

// 32 bytes max
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Compressor(String);

impl From<&str> for Compressor {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for Compressor {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<Compressor> for String {
    fn from(value: Compressor) -> Self {
        value.0
    }
}

impl AsRef<str> for Compressor {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Encode for Compressor {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        let name = self.0.as_bytes();
        let max = name.len().min(31);
        (&name[..max]).encode(buf)?;

        let zero = [0u8; 32];
        (&zero[..32 - max]).encode(buf)
    }
}

impl Decode for Compressor {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let name = <[u8; 32]>::decode(buf)?;

        let name = String::from_utf8_lossy(&name)
            .trim_end_matches('\0')
            .to_string();

        Ok(Self(name))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Zeroed {
    pub size: usize,
}

impl Zeroed {
    pub fn new(size: usize) -> Self {
        Self { size }
    }
}

impl Encode for Zeroed {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        let zero = [0u8; 32];
        let mut size = self.size;

        while size > 0 {
            let len = zero.len().min(size);
            (&zero[..len]).encode(buf)?;
            size -= len;
        }

        Ok(())
    }
}

impl Decode for Zeroed {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let size = buf.remaining();
        buf.advance(size);
        Ok(Self { size })
    }
}

impl From<usize> for Zeroed {
    fn from(size: usize) -> Self {
        Self { size }
    }
}
