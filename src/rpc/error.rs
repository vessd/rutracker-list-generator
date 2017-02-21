use rpc::transmission;
use std::{fmt, error, result};

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Transmission(transmission::Error),
    NotSha1(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Transmission(ref err) => err.fmt(f),
            Error::NotSha1(ref s) => write!(f, "not a valid sha1: {}", s),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Transmission(ref err) => err.description(),
            Error::NotSha1(_) => "not a valid sha1",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::Transmission(ref err) => Some(err),
            Error::NotSha1(_) => None,
        }
    }
}

impl From<transmission::Error> for Error {
    fn from(err: transmission::Error) -> Error {
        Error::Transmission(err)
    }
}
