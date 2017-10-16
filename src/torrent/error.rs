use std::{error, fmt};
use self::Error::*;

#[derive(Debug)]
pub enum Error {
    Rpc(::rpc::Error),
    Api(::rutracker::api::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Rpc(ref err) => err.fmt(f),
            Error::Api(ref err) => err.fmt(f),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Rpc(ref err) => err.description(),
            Error::Api(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::Rpc(ref err) => Some(err),
            Error::Api(ref err) => Some(err),
        }
    }
}

impl From<::rpc::Error> for Error {
    fn from(err: ::rpc::Error) -> Error {
        Rpc(err)
    }
}

impl From<::rutracker::api::Error> for Error {
    fn from(err: ::rutracker::api::Error) -> Error {
        Api(err)
    }
}
