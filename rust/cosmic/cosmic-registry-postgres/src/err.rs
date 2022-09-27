use cosmic_universe::err::UniErr;
use std::io::{Error, ErrorKind};
use std::string::FromUtf8Error;
use cosmic_hyperverse::HyperErr;
use std::str::Utf8Error;
use tokio::sync::mpsc;
use strum::ParseError;
use wasmer::{CompileError, ExportError, InstantiationError, RuntimeError};

#[derive(Debug, Clone)]
pub enum PostErr {
    Dupe,
    Error(String),
}

impl From<std::io::Error> for PostErr {
    fn from(e: Error) -> Self {
        PostErr::Error(e.to_string())
    }
}

impl From<strum::ParseError> for PostErr {
    fn from(x: ParseError) -> Self {
        PostErr::Error("strum parse error".to_string())
    }
}

impl Into<UniErr> for PostErr {
    fn into(self) -> UniErr {
        UniErr::new(500u16, "Post Err")
    }
}

impl From<UniErr> for PostErr {
    fn from(e: UniErr) -> Self {
        PostErr::Error(e.to_string())
    }
}

impl From<Box<bincode::error::ErrorKind>> for PostErr {
    fn from(_: Box<bincode::error::ErrorKind>) -> Self {
        todo!()
    }
}

impl mechtron_host::err::HostErr for PostErr {
    fn to_uni_err(self) -> UniErr {
        todo!()
    }
}

impl From<CompileError> for PostErr {
    fn from(_: CompileError) -> Self {
        todo!()
    }
}

impl From<RuntimeError> for PostErr {
    fn from(_: RuntimeError) -> Self {
        todo!()
    }
}

impl From<Box<bincode::ErrorKind>> for PostErr {
    fn from(_: Box<bincode::ErrorKind>) -> Self {
        todo!()
    }
}

impl From<ExportError> for PostErr {
    fn from(_: ExportError) -> Self {
        todo!()
    }
}

impl From<Utf8Error> for PostErr {
    fn from(_: Utf8Error) -> Self {
        todo!()
    }
}

impl From<FromUtf8Error> for PostErr {
    fn from(_: FromUtf8Error) -> Self {
        todo!()
    }
}

impl From<InstantiationError> for PostErr {
    fn from(_: InstantiationError) -> Self {
        todo!()
    }
}

impl From<zip::result::ZipError> for PostErr {
    fn from(_: zip::result::ZipError) -> Self {
        todo!()
    }
}

impl From<acid_store::Error> for PostErr {
    fn from(_: acid_store::Error) -> Self {
        todo!()
    }
}

impl HyperErr for PostErr {
    fn to_uni_err(&self) -> UniErr {
        UniErr::new(500u16, "Post Err")
    }

    fn new<S>(message: S) -> Self
    where
        S: ToString,
    {
        PostErr::Error(message.to_string())
    }

    fn status_msg<S>(status: u16, message: S) -> Self
    where
        S: ToString,
    {
        PostErr::Error(message.to_string())
    }

    fn status(&self) -> u16 {
        500u16
    }
}

impl ToString for PostErr {
    fn to_string(&self) -> String {
        match self {
            PostErr::Dupe => "Dupe".to_string(),
            PostErr::Error(error) => error.to_string(),
        }
    }
}

impl From<sqlx::Error> for PostErr {
    fn from(e: sqlx::Error) -> Self {
        PostErr::Error(e.to_string())
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for PostErr {
    fn from(e: tokio::sync::oneshot::error::RecvError) -> Self {
        PostErr::Error(format!("{}", e.to_string()))
    }
}

impl<T> From<mpsc::error::SendError<T>> for PostErr {
    fn from(e: mpsc::error::SendError<T>) -> Self {
        PostErr::Error(e.to_string())
    }
}

impl From<&str> for PostErr {
    fn from(e: &str) -> Self {
        PostErr::Error(e.into())
    }
}

impl From<String> for PostErr {
    fn from(e: String) -> Self {
        PostErr::Error(e)
    }
}
