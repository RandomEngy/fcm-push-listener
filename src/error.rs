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
    Request(&'static str, reqwest::Error),
    Response(&'static str, reqwest::Error),
    Base64Decode(&'static str, base64::DecodeError),
    KeyCreation(ece::Error),
    Socket(std::io::Error),
}

impl From<ece::Error> for Error {
    fn from(err: ece::Error) -> Error {
        Error::KeyCreation(err)
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
            Self::ProtobufDecode(kind, e) => write!(f, "Error decoding {kind}: {e}"),
            Self::Base64Decode(kind, e) => write!(f, "Error decoding {kind}: {e}"),
            Self::Request(kind, e) => write!(f, "{kind} API request error: {e}"),
            Self::Response(kind, e) => write!(f, "{kind} API response error: {e}"),
            Error::KeyCreation(..) => write!(f, "Creating encryption keys failed"),
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
            Self::ProtobufDecode(_, ref e) => Some(e),
            Self::Base64Decode(_, ref e) => Some(e),
            Self::Request(_, ref e) => Some(e),
            Self::Response(_, ref e) => Some(e),
            Error::KeyCreation(ref e) => Some(e),
            Error::Socket(ref e) => Some(e),
        }
    }
}
