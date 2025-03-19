use std::io::Cursor;

/// A contiguous buffer of bytes.
// We're not using bytes::Buf because of some strange bugs with take().
pub trait Buf: std::fmt::Debug {
    fn remaining(&self) -> usize;
    fn has_remaining(&self) -> bool {
        self.remaining() > 0
    }

    fn slice(&self, size: usize) -> &[u8];
    fn advance(&mut self, n: usize);
}

impl Buf for &[u8] {
    fn remaining(&self) -> usize {
        self.len()
    }

    fn slice(&self, size: usize) -> &[u8] {
        self[..size].as_ref()
    }

    fn advance(&mut self, n: usize) {
        *self = &self[n..];
    }
}

impl<T: AsRef<[u8]> + std::fmt::Debug> Buf for Cursor<T> {
    fn remaining(&self) -> usize {
        self.get_ref().as_ref().len() - self.position() as usize
    }

    fn slice(&self, size: usize) -> &[u8] {
        let pos = self.position() as usize;
        self.get_ref().as_ref()[pos..pos + size].as_ref()
    }

    fn advance(&mut self, n: usize) {
        self.set_position(self.position() + n as u64);
    }
}

impl<T: Buf + ?Sized> Buf for &mut T {
    fn remaining(&self) -> usize {
        (**self).remaining()
    }

    fn slice(&self, size: usize) -> &[u8] {
        (**self).slice(size)
    }

    fn advance(&mut self, n: usize) {
        (**self).advance(n);
    }
}

#[cfg(feature = "bytes")]
impl Buf for bytes::Bytes {
    fn remaining(&self) -> usize {
        self.len()
    }

    fn slice(&self, size: usize) -> &[u8] {
        &self[..size]
    }

    fn advance(&mut self, n: usize) {
        bytes::Buf::advance(self, n);
    }
}

/// A mutable contiguous buffer of bytes.
// We're not using bytes::BufMut because it doesn't allow seeking backwards (to set the size).
pub trait BufMut: std::fmt::Debug {
    // Returns the current length.
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // Append a slice to the buffer
    fn append_slice(&mut self, val: &[u8]);

    // Set a slice at a position in the buffer.
    fn set_slice(&mut self, pos: usize, val: &[u8]);
}

impl BufMut for Vec<u8> {
    fn len(&self) -> usize {
        self.len()
    }

    fn append_slice(&mut self, v: &[u8]) {
        self.extend_from_slice(v);
    }

    fn set_slice(&mut self, pos: usize, val: &[u8]) {
        self[pos..pos + val.len()].copy_from_slice(val);
    }
}

impl<T: BufMut + ?Sized> BufMut for &mut T {
    fn len(&self) -> usize {
        (**self).len()
    }

    fn append_slice(&mut self, v: &[u8]) {
        (**self).append_slice(v);
    }

    fn set_slice(&mut self, pos: usize, val: &[u8]) {
        (**self).set_slice(pos, val);
    }
}

#[cfg(feature = "bytes")]
impl BufMut for bytes::BytesMut {
    fn len(&self) -> usize {
        self.len()
    }

    fn append_slice(&mut self, v: &[u8]) {
        self.extend_from_slice(v);
    }

    fn set_slice(&mut self, pos: usize, val: &[u8]) {
        self[pos..pos + val.len()].copy_from_slice(val);
    }
}
