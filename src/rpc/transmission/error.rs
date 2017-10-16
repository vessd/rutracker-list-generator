use std::{error, fmt, result};
use self::Error::*;

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Reqwest(::reqwest::Error),
    SerdeJson(::serde_json::Error),
    UrlError(::reqwest::UrlError),
    ParseIdError,
    UnexpectedResponse(::reqwest::StatusCode),
    TransmissionError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Reqwest(ref err) => err.fmt(f),
            SerdeJson(ref err) => err.fmt(f),
            UrlError(ref err) => err.fmt(f),
            ParseIdError => write!(f, "failed to extract a identifier from the response header"),
            UnexpectedResponse(ref s) => write!(
                f,
                "unexpected response from the transmission server: {}",
                s.to_string()
            ),
            TransmissionError(ref s) => write!(f, "the transmission server responded with an error: {}", s),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Reqwest(ref err) => err.description(),
            SerdeJson(ref err) => err.description(),
            UrlError(ref err) => err.description(),
            ParseIdError => "failed to parse id",
            UnexpectedResponse(_) => "unexpected response",
            TransmissionError(_) => "transmission error",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Reqwest(ref err) => Some(err),
            SerdeJson(ref err) => Some(err),
            UrlError(ref err) => Some(err),
            ParseIdError | UnexpectedResponse(_) | TransmissionError(_) => None,
        }
    }
}

impl From<::reqwest::Error> for Error {
    fn from(err: ::reqwest::Error) -> Error {
        Reqwest(err)
    }
}

impl From<::serde_json::Error> for Error {
    fn from(err: ::serde_json::Error) -> Error {
        SerdeJson(err)
    }
}

impl From<::reqwest::UrlError> for Error {
    fn from(err: ::reqwest::UrlError) -> Error {
        UrlError(err)
    }
}
