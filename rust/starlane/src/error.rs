use std::fmt;
use std::fmt::Formatter;
use crate::frame::ProtoFrame;
use tokio::sync::mpsc::error::SendError;
use futures::channel::oneshot::Canceled;
use tokio::sync::broadcast;
use tokio::time::error::Elapsed;

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