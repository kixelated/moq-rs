use super::*;

use crate::{Any, DecodeAtom, Error, Header, Result};

use tokio::io::AsyncRead;

impl AsyncReadFrom for Any {
    async fn read_from<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        <Option<Any> as AsyncReadFrom>::read_from(r)
            .await?
            .ok_or(Error::UnexpectedEof)
    }
}

impl AsyncReadFrom for Option<Any> {
    async fn read_from<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        let header = match Option::<Header>::read_from(r).await? {
            Some(header) => header,
            None => return Ok(None),
        };
        let mut buf = header.read_body_tokio(r).await?;
        Ok(Some(Any::decode_atom(&header, &mut buf)?))
    }
}

impl AsyncReadAtom for Any {
    async fn read_atom<R: AsyncRead + Unpin>(header: &Header, r: &mut R) -> Result<Self> {
        let mut buf = header.read_body_tokio(r).await?;
        Any::decode_atom(header, &mut buf)
    }
}
