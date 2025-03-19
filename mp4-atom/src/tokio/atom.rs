use super::*;

use crate::{Atom, Buf, DecodeAtom, Encode, Error, Header, Result};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

impl<T: Encode> AsyncWriteTo for T {
    async fn write_to<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()> {
        // TODO We should avoid allocating a buffer here.
        let mut buf = Vec::new();
        self.encode(&mut buf)?;
        Ok(w.write_all(&buf).await?)
    }
}

impl<T: Atom> AsyncReadFrom for T {
    async fn read_from<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        <Option<T> as AsyncReadFrom>::read_from(r)
            .await?
            .ok_or(Error::MissingBox(T::KIND))
    }
}

impl<T: Atom> AsyncReadFrom for Option<T> {
    async fn read_from<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        let header = match Option::<Header>::read_from(r).await? {
            Some(header) => header,
            None => return Ok(None),
        };

        let mut buf = header.read_body_tokio(r).await?;

        let atom = match T::decode_body(&mut buf) {
            Ok(atom) => atom,
            Err(Error::OutOfBounds) => return Err(Error::OverDecode(T::KIND)),
            Err(Error::ShortRead) => return Err(Error::UnderDecode(T::KIND)),
            Err(err) => return Err(err),
        };

        if buf.has_remaining() {
            return Err(Error::UnderDecode(T::KIND));
        }

        Ok(Some(atom))
    }
}

impl<T: Atom> AsyncReadUntil for T {
    async fn read_until<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        Option::<T>::read_until(r)
            .await?
            .ok_or(Error::MissingBox(T::KIND))
    }
}

impl<T: Atom> AsyncReadUntil for Option<T> {
    async fn read_until<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        while let Some(header) = Option::<Header>::read_from(r).await? {
            if header.kind == T::KIND {
                let mut buf = header.read_body_tokio(r).await?;
                return Ok(Some(T::decode_atom(&header, &mut buf)?));
            }
        }

        Ok(None)
    }
}

impl<T: Atom> AsyncReadAtom for T {
    async fn read_atom<R: AsyncRead + Unpin>(header: &Header, r: &mut R) -> Result<Self> {
        if header.kind != T::KIND {
            return Err(Error::UnexpectedBox(header.kind));
        }

        let mut buf = header.read_body_tokio(r).await?;
        Self::decode_atom(header, &mut buf)
    }
}
