use std::error;

#[derive(Debug)]
pub enum Error {
    /// Dependency failed, i.e. we blame them
    DependencyFailure(&'static str, &'static str),
    /// Dependency rejection, i.e. they blame us
    DependencyRejection(&'static str, String),
    /// Received an encrypted message with no decryption params
    MissingCryptoMetadata(&'static str),
    /// Protobuf deserialization failure, probably a contract change
    ProtobufDecode(&'static str, prost::DecodeError),
    Base64Decode(base64::DecodeError),
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

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::DependencyFailure(api, problem) => write!(f, "{api} API {problem}"),
            Self::DependencyRejection(api, reason) => {
                write!(f, "{api} API rejected request: {reason}")
            }
            Self::MissingCryptoMetadata(kind) => write!(f, "Missing {kind} metadata on message"),
            Error::ProtobufDecode(kind, _) => write!(f, "Error decoding {kind}"),
            Error::Base64Decode(..) => write!(f, "Error decoding base64 string"),
            Error::KeyCreation(..) => write!(f, "Creating encryption keys failed"),
            Error::Http(..) => write!(f, "Register HTTP call failed"),
            Error::Socket(..) => write!(f, "TCP socket failed"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            Self::DependencyFailure(_, _) => None,
            Self::DependencyRejection(_, _) => None,
            Self::MissingCryptoMetadata(_) => None,
            Error::ProtobufDecode(_, ref e) => Some(e),
            Error::Base64Decode(ref e) => Some(e),
            Error::KeyCreation(ref e) => Some(e),
            Error::Http(ref e) => Some(e),
            Error::Socket(ref e) => Some(e),
        }
    }
}
