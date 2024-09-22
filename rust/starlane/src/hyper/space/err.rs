use ascii::FromAsciiError;
use starlane_space::err::SpaceErr;
use starlane_space::substance::Substance;
use starlane_space::wave::core::http2::StatusCode;
use starlane_space::wave::core::ReflectedCore;
use std::fmt::Debug;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ErrKind {
    Default,
    Dupe,
    Status(u16),
}

/*
#[derive(Debug, Clone)]
pub struct CosmicErr {
    pub kind: ErrKind,
    pub message: String,
}

 */

pub trait HyperErr:
    Sized
    + Debug
    + Send
    + Sync
    + ToString
    + Clone
    + Into<SpaceErr>
    + From<SpaceErr>
    + From<String>
    + From<&'static str>
    + From<tokio::sync::oneshot::error::RecvError>
    + From<std::io::Error>
    + From<zip::result::ZipError>
    + From<Box<bincode::ErrorKind>>
    + From<strum::ParseError>
    + From<url::ParseError>
    + From<FromAsciiError<std::string::String>>
    + From<SpaceErr>
    + Into<SpaceErr>
    + From<()>
{
    fn to_space_err(&self) -> SpaceErr;

    fn new<S>(message: S) -> Self
    where
        S: ToString;

    fn status_msg<S>(status: u16, message: S) -> Self
    where
        S: ToString;

    fn not_found() -> Self {
        Self::not_found_msg("Not Found")
    }

    fn not_found_msg<S>(message: S) -> Self
    where
        S: ToString,
    {
        Self::status_msg(404, message)
    }

    fn status(&self) -> u16;

    fn as_reflected_core(&self) -> ReflectedCore {
        let mut core = ReflectedCore::new();
        core.status =
            StatusCode::from_u16(self.status()).unwrap_or(StatusCode::from_u16(500u16).unwrap());
        core.body = Substance::Empty;
        core
    }

    fn kind(&self) -> ErrKind;

    fn with_kind<S>(kind: ErrKind, msg: S) -> Self
    where
        S: ToString;
}

/*
impl HyperErr for CosmicErr {
    fn to_space_err(&self) -> SpaceErr {
        SpaceErr::Status {
            status: 0,
            message: self.message.to_string(),
        }
    }

    fn new<S>(message: S) -> Self
    where
        S: ToString,
    {
        Self {
            message,
            kind: ErrKind::Default,
        }
    }

    fn status_msg<S>(status: u16, message: S) -> Self
    where
        S: ToString,
    {
        Self {
            kind: ErrKind::Status(status),
            message: message.to_string(),
        }
    }

    fn status(&self) -> u16 {
        match &self.kind {
            ErrKind::Status(s) => s.clone(),
            _ => 0u16,
        }
    }

    fn kind(&self) -> ErrKind {
        self.kind.clone()
    }

    fn with_kind<S>(kind: ErrKind, msg: S) -> Self
    where
        S: ToString,
    {
        let message = msg.to_string();
        Self { kind, message }
    }
}

 */

use std::io;
use std::str::Utf8Error;
use std::string::FromUtf8Error;
use tokio::sync::oneshot;
use tokio::time::error::Elapsed;
use wasmer::{CompileError, ExportError, InstantiationError, RuntimeError};

#[derive(Debug, Clone)]
pub struct Error {
    pub message: String,
    pub kind: ErrKind,
}

impl Error {
    pub fn new<S: ToString>(message: S) -> Self {
        Self {
            message: message.to_string(),
            kind: ErrKind::Default,
        }
    }
}

impl ToString for Error {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}

impl From<()> for Error {
    fn from(_: ()) -> Self {
        Error::new("Empty")
    }
}

impl From<strum::ParseError> for Error {
    fn from(e: strum::ParseError) -> Self {
        Self {
            kind: ErrKind::Default,
            message: e.to_string(),
        }
    }
}

impl From<url::ParseError> for Error {
    fn from(e: url::ParseError) -> Self {
        Self {
            kind: ErrKind::Default,
            message: e.to_string(),
        }
    }
}
impl From<FromAsciiError<std::string::String>> for Error {
    fn from(e: FromAsciiError<String>) -> Self {
        Self {
            kind: ErrKind::Default,
            message: e.to_string(),
        }
    }
}

impl HyperErr for Error {
    fn to_space_err(&self) -> SpaceErr {
        SpaceErr::server_error(self.to_string())
    }

    fn new<S>(message: S) -> Self
    where
        S: ToString,
    {
        Error::new(message)
    }

    fn status_msg<S>(status: u16, message: S) -> Self
    where
        S: ToString,
    {
        Error::new(message)
    }

    fn status(&self) -> u16 {
        if let ErrKind::Status(code) = self.kind {
            code
        } else {
            500u16
        }
    }

    fn kind(&self) -> ErrKind {
        self.kind.clone()
    }

    fn with_kind<S>(kind: ErrKind, msg: S) -> Self
    where
        S: ToString,
    {
        Error {
            kind,
            message: msg.to_string(),
        }
    }
}
impl Into<SpaceErr> for Error {
    fn into(self) -> SpaceErr {
        SpaceErr::server_error(self.to_string())
    }
}

impl From<oneshot::error::RecvError> for Error {
    fn from(err: oneshot::error::RecvError) -> Self {
        Error::new(err)
    }
}

impl From<Elapsed> for Error {
    fn from(err: Elapsed) -> Self {
        Error::new(err)
    }
}

impl From<String> for Error {
    fn from(err: String) -> Self {
        Error::new(err)
    }
}

impl From<&'static str> for Error {
    fn from(err: &'static str) -> Self {
        Error::new(err)
    }
}

impl From<SpaceErr> for Error {
    fn from(err: SpaceErr) -> Self {
        Error::new(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::new(err)
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(a: zip::result::ZipError) -> Self {
        Error::new(a)
    }
}

impl From<Box<bincode::ErrorKind>> for Error {
    fn from(e: Box<bincode::ErrorKind>) -> Self {
        Error::new(e)
    }
}

impl From<ExportError> for Error {
    fn from(e: ExportError) -> Self {
        Error::new(e)
    }
}

impl From<Utf8Error> for Error {
    fn from(e: Utf8Error) -> Self {
        Error::new(e)
    }
}

impl From<FromUtf8Error> for Error {
    fn from(e: FromUtf8Error) -> Self {
        Error::new(e)
    }
}

impl From<InstantiationError> for Error {
    fn from(_: InstantiationError) -> Self {
        todo!()
    }
}

impl From<CompileError> for Error {
    fn from(e: CompileError) -> Self {
        Error::new(e)
    }
}

impl From<RuntimeError> for Error {
    fn from(e: RuntimeError) -> Self {
        Error::new(e)
    }
}
