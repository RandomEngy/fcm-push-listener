use std::error;
use std::{fmt, string::FromUtf8Error};

#[derive(Debug)]
pub enum Error {
    MissingMessagePayload,
    MissingCryptoMetadata,
    ProtobufDecode(prost::DecodeError),
    Base64Decode(base64::DecodeError),
    FromUtf8(FromUtf8Error),
    InvalidResponse(String),
    ServerError(String),
    KeyCreation(ece::Error),
    Http(reqwest::Error),
    Socket(std::io::Error),
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Error {
        Error::Http(err)
    }
}

impl From<ece::Error> for Error {
    fn from(err: ece::Error) -> Error {
        Error::KeyCreation(err)
    }
}

impl From<FromUtf8Error> for Error {
    fn from(err: FromUtf8Error) -> Error {
        Error::FromUtf8(err)
    }
}

impl From<prost::DecodeError> for Error {
    fn from(err: prost::DecodeError) -> Error {
        Error::ProtobufDecode(err)
    }
}

impl From<base64::DecodeError> for Error {
    fn from(err: base64::DecodeError) -> Error {
        Error::Base64Decode(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::Socket(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::MissingMessagePayload => write!(f, "Message payload is missing"),
            Error::MissingCryptoMetadata => write!(f, "Missing crypto metadata on message"),
            Error::ProtobufDecode(..) => write!(f, "Error decoding response"),
            Error::Base64Decode(..) => write!(f, "Error decoding base64 string"),
            Error::FromUtf8(..) => write!(f, "Error getting string from UTF8"),
            Error::InvalidResponse(url) => write!(f, "Response from call to {} was invalid", url),
            Error::ServerError(details) => write!(f, "Error from server: {}", details),
            Error::KeyCreation(..) => write!(f, "Creating encryption keys failed"),
            Error::Http(..) => write!(f, "Register HTTP call failed"),
            Error::Socket(..) => write!(f, "TCP socket failed"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            Error::MissingMessagePayload => None,
            Error::MissingCryptoMetadata => None,
            Error::ProtobufDecode(ref e) => Some(e),
            Error::Base64Decode(ref e) => Some(e),
            Error::FromUtf8(ref e) => Some(e),
            Error::InvalidResponse(ref _e) => None,
            Error::ServerError(ref _e) => None,
            Error::KeyCreation(ref e) => Some(e),
            Error::Http(ref e) => Some(e),
            Error::Socket(ref e) => Some(e),
        }
    }
}
