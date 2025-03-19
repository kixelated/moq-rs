use std::io::{Read, Write};

use super::*;

/// Read a type from a reader.
pub trait ReadFrom: Sized {
    fn read_from<R: Read>(r: &mut R) -> Result<Self>;
}

/// Read an atom from a reader provided the header.
pub trait ReadAtom: Sized {
    fn read_atom<R: Read>(header: &Header, r: &mut R) -> Result<Self>;
}

/// Keep discarding atoms until the desired atom is found.
pub trait ReadUntil: Sized {
    fn read_until<R: Read>(r: &mut R) -> Result<Self>;
}

/// Write a type to a writer.
pub trait WriteTo {
    fn write_to<W: Write>(&self, w: &mut W) -> Result<()>;
}

impl<T: Encode> WriteTo for T {
    fn write_to<W: Write>(&self, w: &mut W) -> Result<()> {
        // TODO We should avoid allocating a buffer here.
        let mut buf = Vec::new();
        self.encode(&mut buf)?;
        Ok(w.write_all(&buf)?)
    }
}
