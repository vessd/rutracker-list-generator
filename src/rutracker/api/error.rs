use std::{error, fmt, result};
use self::Error::*;
use super::ResponseError;

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    UrlError(::reqwest::UrlError),
    Reqwest(::reqwest::Error),
    ApiError(&'static str, ResponseError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            UrlError(ref err) => err.fmt(f),
            Reqwest(ref err) => err.fmt(f),
            ApiError(method, ref err) => write!(
                f,
                "{}: {{ code: {}, text: {} }}",
                method,
                err.code,
                err.text
            ),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            UrlError(ref err) => err.description(),
            Reqwest(ref err) => err.description(),
            ApiError(_, _) => "Rutracker API error",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            UrlError(ref err) => Some(err),
            Reqwest(ref err) => Some(err),
            ApiError(_, _) => None,
        }
    }
}

impl From<::reqwest::UrlError> for Error {
    fn from(err: ::reqwest::UrlError) -> Error {
        UrlError(err)
    }
}

impl From<::reqwest::Error> for Error {
    fn from(err: ::reqwest::Error) -> Error {
        Reqwest(err)
    }
}
