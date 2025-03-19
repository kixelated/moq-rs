use crate::FourCC;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("out of bounds")]
    OutOfBounds,

    #[error("short read")]
    ShortRead,

    #[error("over decode: {0}")]
    OverDecode(FourCC),

    #[error("under decode: {0}")]
    UnderDecode(FourCC),

    #[error("atom too large")]
    TooLarge(FourCC),

    #[error("invalid size")]
    InvalidSize,

    #[error("invalid fourcc")]
    InvalidFourCC,

    #[error("unknown version: {0}")]
    UnknownVersion(u8),

    #[error("invalid string: {0}")]
    InvalidString(String),

    #[error("missing box: {0}")]
    MissingBox(FourCC),

    #[error("unexpected box: {0}")]
    UnexpectedBox(FourCC),

    #[error("duplicate box: {0}")]
    DuplicateBox(FourCC),

    #[error("missing descriptor: {0}")]
    MissingDescriptor(u8),

    #[error("unexpected descriptor: {0}")]
    UnexpectedDescriptor(u8),

    #[error("unexpected eof")]
    UnexpectedEof,

    // Returned in the rare case when we can't represent a value in the desired type
    #[error("out of memory")]
    OutOfMemory,

    #[error("reserved")]
    Reserved,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
