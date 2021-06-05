use std::fmt;
use std::fmt::{Formatter, Display};
use crate::frame::ProtoFrame;
use tokio::sync::mpsc::error::SendError;
use futures::channel::oneshot::Canceled;
use tokio::sync::broadcast;
use tokio::time::error::Elapsed;
use semver::SemVerError;
use base64::DecodeError;
use crate::message::Fail;
use std::string::FromUtf8Error;
use std::convert::TryFrom;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Error{
    pub error: String
}



impl fmt::Display for Error{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}",self.error)
    }
}




impl From<Elapsed> for Error{
    fn from(e: Elapsed) -> Self {
        Error{
            error: format!("{}",e.to_string())
        }
    }
}

impl From<validate::Error> for Error{
    fn from(e: validate::Error) -> Self {
        Error{
            error: format!("{}",e.get_message())
        }
    }
}

impl From<uuid::Error> for Error {
    fn from(e: uuid::Error) -> Self {
        e.to_string().into()
    }
}

impl From<FromUtf8Error> for Error{
    fn from(e: FromUtf8Error) -> Self {
        Error{
            error: e.to_string()
        }
    }
}

impl From<Fail> for Error{
    fn from(fail: Fail) -> Self {
        Error{
            error: format!("{}",fail.to_string())
        }
    }
}

impl From<()> for Error{
    fn from(e: ()) -> Self {
        Error{
            error: "() Error".to_string()
        }
    }
}

impl From<bincode::ErrorKind> for Error{
    fn from(e: bincode::ErrorKind) -> Self {
        Error{
            error: format!("{}",e.to_string())
        }
    }
}

impl From<Box<bincode::ErrorKind>> for Error{
    fn from(e: Box<bincode::ErrorKind>) -> Self {
        Error{
            error: format!("{}",e.to_string())
        }
    }
}

impl From<DecodeError> for Error{
    fn from(e: DecodeError) -> Self {
        Error{
            error: format!("{}",e.to_string())
        }
    }
}

impl From<SemVerError> for Error{
    fn from(e: SemVerError) -> Self {
        Error{
            error: format!("{}",e.to_string())
        }
    }
}

impl From<Error> for rusqlite::Error{
    fn from(e: Error) -> Self {
        rusqlite::Error::InvalidQuery
    }
}

impl From<rusqlite::Error> for Error{
    fn from(e: rusqlite::Error) -> Self {
        Error{
            error: format!("{}",e.to_string())
        }
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for Error{
    fn from(e: tokio::sync::oneshot::error::RecvError) -> Self {
        Error{
            error: format!("{}",e.to_string())
        }
    }
}

impl <E> From<broadcast::error::SendError<E>> for Error{
    fn from(e: broadcast::error::SendError<E>) -> Self {
        Error{
            error: format!("{}",e.to_string())
        }
    }
}


impl From<&str> for Error{
    fn from(e: &str) -> Self {
        Error{
            error: format!("{:?}",e)
        }
    }
}

impl From<String> for Error{
    fn from(e: String) -> Self {
        Error{
            error: format!("{:?}",e)
        }
    }
}

impl  From<Canceled> for Error{
    fn from(e: Canceled) -> Self {
        Error{
            error: format!("{}",e)
        }
    }
}

impl <T> From<SendError<T>> for Error{
    fn from(e: SendError<T>) -> Self {
        Error{
            error: format!("{}",e)
        }
    }
}