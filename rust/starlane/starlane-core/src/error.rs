use std::convert::Infallible;
use std::env::VarError;
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::num::ParseIntError;
use std::string::FromUtf8Error;
use std::sync::PoisonError;

use base64::DecodeError;
use futures::channel::oneshot::Canceled;
use nom::error::VerboseError;
use semver::SemVerError;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::error::{SendError, TrySendError};
use tokio::time::error::Elapsed;
use zip::result::ZipError;

use crate::fail::Fail;
use actix_web::ResponseError;
use alcoholic_jwt::ValidationError;
use ascii::FromAsciiError;
use cosmic_api::error::StatusErr;
use cosmic_nom::Span;
use handlebars::RenderError;
use http::header::{InvalidHeaderName, InvalidHeaderValue, ToStrError};
use http::method::InvalidMethod;
use http::status::InvalidStatusCode;
use http::uri::InvalidUri;
use keycloak::KeycloakError;
use mesh_portal::error::MsgErr;
use nom_supreme::error::ErrorTree;
use sqlx::error::DatabaseError;
use tokio::task::JoinError;
use wasmer::{CompileError, ExportError, RuntimeError};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Error {
    pub status: u16,
    pub message: String,
}

impl Error {
    pub fn from_internal<E: ToString>(err: E) -> Self {
        Self {
            status: 500,
            message: err.to_string(),
        }
    }
}

impl StatusErr for Error {
    fn status(&self) -> u16 {
        self.status.clone()
    }

    fn message(&self) -> String {
        self.message.clone()
    }
}

impl std::error::Error for Error {}

impl Error {
    pub fn new(message: &str) -> Self {
        Self::from_internal(message)
    }

    pub fn with_status(status: u16, message: &str) -> Self {
        Self {
            status,
            message: message.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<ZipError> for Error {
    fn from(err: ZipError) -> Self {
        match err {
            ZipError::Io(io) => Error::from_internal(io),
            ZipError::InvalidArchive(err) => Error::from_internal(err),
            ZipError::UnsupportedArchive(un) => Error::from_internal(un),
            ZipError::FileNotFound => Error::from_internal("ZipError: FileNotFound"),
        }
    }
}

impl From<kube::Error> for Error {
    fn from(err: kube::Error) -> Self {
        Error::from_internal(err)
    }
}

impl<T> From<tokio::sync::mpsc::error::TrySendError<T>> for Error {
    fn from(err: tokio::sync::mpsc::error::TrySendError<T>) -> Self {
        Error::from_internal(err)
    }
}

impl From<tokio::sync::broadcast::error::RecvError> for Error {
    fn from(err: RecvError) -> Self {
        Error::from_internal(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::from_internal(err)
    }
}

impl From<wasm_membrane_host::error::Error> for Error {
    fn from(err: wasm_membrane_host::error::Error) -> Self {
        Error::from_internal(err)
    }
}
impl From<CompileError> for Error {
    fn from(err: CompileError) -> Self {
        Error::from_internal(err)
    }
}

impl From<nom::Err<VerboseError<&str>>> for Error {
    fn from(err: nom::Err<VerboseError<&str>>) -> Self {
        Error::from_internal(err)
    }
}

/*
impl From<nom::Err<ErrorTree<&str>>> for Error {
    fn from(err: nom::Err<ErrorTree<&str>>) -> Self {
        Error::from_internal(err)
    }
}

 */

impl<I: Span + core::fmt::Debug> From<nom::Err<ErrorTree<I>>> for Error {
    fn from(err: nom::Err<ErrorTree<I>>) -> Self {
        Error::from_internal(err)
    }
}

impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        Error::from_internal(err)
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(err: serde_yaml::Error) -> Self {
        Error::from_internal(err)
    }
}

impl From<Infallible> for Error {
    fn from(err: Infallible) -> Self {
        Error::from_internal(err)
    }
}

impl From<VarError> for Error {
    fn from(err: VarError) -> Self {
        Error::from_internal(err)
    }
}

impl From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Self {
        Error::from_internal(err)
    }
}

impl From<ParseIntError> for Error {
    fn from(err: ParseIntError) -> Self {
        Error::from_internal(err)
    }
}

impl From<notify::Error> for Error {
    fn from(err: notify::Error) -> Self {
        Error::from_internal(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::from_internal(err)
    }
}

impl From<Elapsed> for Error {
    fn from(err: Elapsed) -> Self {
        Error::from_internal(err)
    }
}

impl From<validate::Error> for Error {
    fn from(err: validate::Error) -> Self {
        Error::from_internal(err.get_message())
    }
}

impl From<uuid::Error> for Error {
    fn from(err: uuid::Error) -> Self {
        Error::from_internal(err)
    }
}

impl From<FromUtf8Error> for Error {
    fn from(err: FromUtf8Error) -> Self {
        Error::from_internal(err)
    }
}

impl From<Fail> for Error {
    fn from(err: Fail) -> Self {
        Error::from_internal(err)
    }
}

impl From<()> for Error {
    fn from(err: ()) -> Self {
        Error::from_internal("() error")
    }
}

impl From<httparse::Error> for Error {
    fn from(err: httparse::Error) -> Self {
        Error::from_internal(err)
    }
}

impl From<bincode::ErrorKind> for Error {
    fn from(err: bincode::ErrorKind) -> Self {
        Error::from_internal(err)
    }
}

impl From<Box<bincode::ErrorKind>> for Error {
    fn from(err: Box<bincode::ErrorKind>) -> Self {
        Error::from_internal(err)
    }
}

impl From<DecodeError> for Error {
    fn from(err: DecodeError) -> Self {
        Error::from_internal(err)
    }
}

impl From<SemVerError> for Error {
    fn from(err: SemVerError) -> Self {
        Error::from_internal(err)
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for Error {
    fn from(err: tokio::sync::oneshot::error::RecvError) -> Self {
        Error::from_internal(err)
    }
}

impl<E> From<broadcast::error::SendError<E>> for Error {
    fn from(err: broadcast::error::SendError<E>) -> Self {
        Error::from_internal(err)
    }
}

impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Error::from_internal(err)
    }
}

impl From<String> for Error {
    fn from(err: String) -> Self {
        Error::from_internal(err)
    }
}

impl From<Canceled> for Error {
    fn from(err: Canceled) -> Self {
        Error::from_internal(err)
    }
}

impl<T> From<SendError<T>> for Error {
    fn from(err: SendError<T>) -> Self {
        Error::from_internal(err)
    }
}

impl From<strum::ParseError> for Error {
    fn from(err: strum::ParseError) -> Self {
        Error::from_internal(err)
    }
}

impl From<RenderError> for Error {
    fn from(err: RenderError) -> Self {
        Error::from_internal(err)
    }
}

impl From<regex::Error> for Error {
    fn from(err: regex::Error) -> Self {
        Error::from_internal(err)
    }
}

impl From<ExportError> for Error {
    fn from(err: ExportError) -> Self {
        Error::from_internal(err)
    }
}

impl From<RuntimeError> for Error {
    fn from(err: RuntimeError) -> Self {
        Error::from_internal(err)
    }
}

impl Into<mesh_portal::version::latest::fail::Fail> for Error {
    fn into(self) -> mesh_portal::version::latest::fail::Fail {
        mesh_portal::version::latest::fail::Fail::Error(self.to_string())
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::from_internal(err)
    }
}

impl From<JoinError> for Error {
    fn from(err: JoinError) -> Self {
        Error::from_internal(err)
    }
}

impl From<KeycloakError> for Error {
    fn from(err: KeycloakError) -> Self {
        Error::from_internal(err)
    }
}

impl From<InvalidMethod> for Error {
    fn from(err: InvalidMethod) -> Self {
        Error::from_internal(err)
    }
}

impl From<InvalidHeaderName> for Error {
    fn from(err: InvalidHeaderName) -> Self {
        Error::from_internal(err)
    }
}

impl From<InvalidHeaderValue> for Error {
    fn from(err: InvalidHeaderValue) -> Self {
        Error::from_internal(err)
    }
}

impl From<ToStrError> for Error {
    fn from(err: ToStrError) -> Self {
        Error::from_internal(err)
    }
}

impl From<serde_urlencoded::de::Error> for Error {
    fn from(err: serde_urlencoded::de::Error) -> Self {
        Error::from_internal(err)
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::from_internal(err)
    }
}

impl From<alcoholic_jwt::ValidationError> for Error {
    fn from(err: alcoholic_jwt::ValidationError) -> Self {
        match err {
            ValidationError::InvalidComponents => Self::with_status(400, "Invalid Jwt Components"),
            ValidationError::InvalidBase64(err) => Self::with_status(
                400,
                format!("Invalid Jwt Base64: {}", err.to_string()).as_str(),
            ),
            ValidationError::InvalidJWK => Self::with_status(500, "Invalid Jwk"),
            ValidationError::InvalidSignature => Self::with_status(400, "Invalid Signature"),
            ValidationError::OpenSSL(err) => {
                Self::with_status(500, format!("OpenSSL: {}", err.to_string()).as_str())
            }
            ValidationError::JSON(json) => {
                Self::with_status(400, format!("JSON : {}", json.to_string()).as_str())
            }
            ValidationError::InvalidClaims(claim) => Self::with_status(400, "Invalid claims"),
        }
    }
}

impl From<InvalidUri> for Error {
    fn from(err: InvalidUri) -> Self {
        Error::from_internal(err)
    }
}

impl From<http::Error> for Error {
    fn from(err: http::Error) -> Self {
        Error::from_internal(err)
    }
}

/*impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Error::from_internal("rusqlite::Error")
    }
}

 */

impl From<FromAsciiError<&str>> for Error {
    fn from(err: FromAsciiError<&str>) -> Self {
        Error::from_internal(err)
    }
}

impl Into<MsgErr> for Error {
    fn into(self) -> MsgErr {
        MsgErr::new(500, self.message.as_str())
    }
}

impl From<MsgErr> for Error {
    fn from(err: MsgErr) -> Self {
        Error::from_internal(err)
    }
}
impl From<InvalidStatusCode> for Error {
    fn from(error: InvalidStatusCode) -> Self {
        Error::from_internal(error)
    }
}

impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        match error.as_database_error() {
            None => Error::from_internal(format!("{:?}", error)),
            Some(err) => Error::from_internal(err.message()),
        }
    }
}
