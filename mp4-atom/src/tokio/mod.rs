mod any;
mod atom;
mod header;
mod traits;

// Tokio versions of any read/write traits
use crate::{Header, Result};

pub trait AsyncReadFrom: Sized {
    #[allow(async_fn_in_trait)]
    async fn read_from<R: tokio::io::AsyncRead + Unpin>(r: &mut R) -> Result<Self>;
}

pub trait AsyncWriteTo {
    #[allow(async_fn_in_trait)]
    async fn write_to<W: tokio::io::AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()>;
}

pub trait AsyncReadAtom: Sized {
    #[allow(async_fn_in_trait)]
    async fn read_atom<R: tokio::io::AsyncRead + Unpin>(header: &Header, r: &mut R)
        -> Result<Self>;
}

pub trait AsyncReadUntil: Sized {
    #[allow(async_fn_in_trait)]
    async fn read_until<R: tokio::io::AsyncRead + Unpin>(r: &mut R) -> Result<Self>;
}
