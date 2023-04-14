use std::io;
use quiche::h3::webtransport;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Quiche(quiche::Error),
    WebTransport(webtransport::Error),
    Server(Server),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<quiche::Error> for Error {
    fn from(err: quiche::Error) -> Error {
        Error::Quiche(err)
    }
}

impl From<webtransport::Error> for Error {
    fn from(err: webtransport::Error) -> Error {
        Error::WebTransport(err)
    }
}

// Custom server error messages.
#[derive(Debug)]
pub enum Server {
    InvalidToken,
    InvalidConnectionID,
    UnknownConnectionID,
}

impl From<Server> for Error {
    fn from(err: Server) -> Error {
        Error::Server(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;