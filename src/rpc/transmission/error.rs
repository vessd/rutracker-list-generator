use std::{io, fmt, error, result};
use self::Error::*;

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Reqwest(::reqwest::Error),
    SerdeJson(::serde_json::Error),
    Ipv6,
    ParseIdError,
    UnexpectedResponse(::reqwest::StatusCode),
    TransmissionError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Io(ref err) => err.fmt(f),
            Reqwest(ref err) => err.fmt(f),
            SerdeJson(ref err) => err.fmt(f),
            Ipv6 => write!(f, "transmission doesn't support Ipv6"),
            ParseIdError => write!(f, "failed to extract a identifier from the response header"),
            UnexpectedResponse(ref s) => {
                write!(f,
                       "unexpected response from the transmission server: {}",
                       s.to_string())
            }
            TransmissionError(ref s) => {
                write!(f, "the transmission server responded with an error: {}", s)
            }
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Io(ref err) => err.description(),
            Reqwest(ref err) => err.description(),
            SerdeJson(ref err) => err.description(),
            Ipv6 => "ipv6 isn't supported",
            ParseIdError => "failed to parse id",
            UnexpectedResponse(_) => "unexpected response",
            TransmissionError(_) => "transmission error",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Io(ref err) => Some(err),
            Reqwest(ref err) => Some(err),
            SerdeJson(ref err) => Some(err),
            Ipv6 |
            ParseIdError |
            UnexpectedResponse(_) |
            TransmissionError(_) => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<::reqwest::Error> for Error {
    fn from(err: ::reqwest::Error) -> Error {
        Error::Reqwest(err)
    }
}

impl From<::hyper::Error> for Error {
    fn from(err: ::hyper::Error) -> Error {
        Error::Reqwest(::reqwest::Error::from(err))
    }
}

impl From<::serde_json::Error> for Error {
    fn from(err: ::serde_json::Error) -> Error {
        Error::SerdeJson(err)
    }
}
