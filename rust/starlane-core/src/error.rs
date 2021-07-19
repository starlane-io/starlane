use crate::frame::ProtoFrame;
use crate::message::Fail;
use base64::DecodeError;
use futures::channel::oneshot::Canceled;
use semver::SemVerError;
use std::convert::{TryFrom, Infallible};
use std::env::VarError;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::num::ParseIntError;
use std::string::FromUtf8Error;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::mpsc::error::{SendError, TrySendError};
use tokio::time::error::Elapsed;
use zip::result::ZipError;
use tokio::sync::broadcast::error::RecvError;
use nom::error::VerboseError;

#[derive(Debug, Clone,Eq,PartialEq)]
pub struct Error {
    pub error: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl From<ZipError> for Error {
    fn from(e: ZipError) -> Self {
        match e {
            ZipError::Io(io) => Error {
                error: io.to_string(),
            },
            ZipError::InvalidArchive(err) => Error {
                error: err.to_string(),
            },
            ZipError::UnsupportedArchive(un) => Error {
                error: un.to_string(),
            },
            ZipError::FileNotFound => Error {
                error: "ZipError: FileNotFound".to_string(),
            },
        }
    }
}


impl From<starlane_resources::error::Error> for Error {
    fn from(e: starlane_resources::error::Error) -> Self {
        e.to_string().into()
    }
}


impl From<kube::Error> for Error {
    fn from(e: kube::Error) -> Self {
        e.to_string().into()
    }
}


impl <T> From<tokio::sync::mpsc::error::TrySendError<T>> for Error {
    fn from(e: TrySendError<T>) -> Self {
        e.to_string().into()
    }
}

impl From<tokio::sync::broadcast::error::RecvError> for Error {
    fn from(e: RecvError) -> Self {
        e.to_string().into()
    }
}

impl From<serde_json::Error> for Error {
    fn from(i: serde_json::Error) -> Self {
        Error {
            error: format!("{}", i )
        }
    }
}


impl From<nom::Err<VerboseError<&str>>> for Error {
    fn from(i: nom::Err<VerboseError<&str>>) -> Self {
        Error {
            error: format!("{}", i.to_string())
        }
    }
}

impl <T> From<std::sync::PoisonError<T>> for Error {
    fn from(i: std::sync::PoisonError<T>) -> Self {
        Error {
            error: format!("{}",i.to_string())
        }
    }
}
impl From<Infallible> for Error {
    fn from(i: Infallible) -> Self {
        Error {
            error: format!("{}", i.to_string())
        }
    }
}

impl From<VarError> for Error {
    fn from(e: VarError) -> Self {
        Error {
            error: format!("{}", e.to_string()),
        }
    }
}

impl From<url::ParseError> for Error {
    fn from(e: url::ParseError) -> Self {
        Error {
            error: format!("{}", e.to_string()),
        }
    }
}

impl From<ParseIntError> for Error {
    fn from(e: ParseIntError) -> Self {
        Error {
            error: format!("{}", e.to_string()),
        }
    }
}

impl From<notify::Error> for Error {
    fn from(e: notify::Error) -> Self {
        Error {
            error: format!("{}", e.to_string()),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error {
            error: format!("{}", e.to_string()),
        }
    }
}

impl From<Elapsed> for Error {
    fn from(e: Elapsed) -> Self {
        Error {
            error: format!("{}", e.to_string()),
        }
    }
}

impl From<validate::Error> for Error {
    fn from(e: validate::Error) -> Self {
        Error {
            error: format!("{}", e.get_message()),
        }
    }
}

impl From<uuid::Error> for Error {
    fn from(e: uuid::Error) -> Self {
        e.to_string().into()
    }
}

impl From<FromUtf8Error> for Error {
    fn from(e: FromUtf8Error) -> Self {
        Error {
            error: e.to_string(),
        }
    }
}

impl From<Fail> for Error {
    fn from(fail: Fail) -> Self {
        Error {
            error: format!("{}", fail.to_string()),
        }
    }
}

impl From<()> for Error {
    fn from(e: ()) -> Self {
        Error {
            error: "() Error".to_string(),
        }
    }
}

impl From<bincode::ErrorKind> for Error {
    fn from(e: bincode::ErrorKind) -> Self {
        Error {
            error: format!("{}", e.to_string()),
        }
    }
}

impl From<Box<bincode::ErrorKind>> for Error {
    fn from(e: Box<bincode::ErrorKind>) -> Self {
        Error {
            error: format!("{}", e.to_string()),
        }
    }
}

impl From<DecodeError> for Error {
    fn from(e: DecodeError) -> Self {
        Error {
            error: format!("{}", e.to_string()),
        }
    }
}

impl From<SemVerError> for Error {
    fn from(e: SemVerError) -> Self {
        Error {
            error: format!("{}", e.to_string()),
        }
    }
}

impl From<Error> for rusqlite::Error {
    fn from(e: Error) -> Self {
        rusqlite::Error::InvalidQuery
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error {
            error: format!("{}", e.to_string()),
        }
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for Error {
    fn from(e: tokio::sync::oneshot::error::RecvError) -> Self {
        Error {
            error: format!("{}", e.to_string()),
        }
    }
}

impl<E> From<broadcast::error::SendError<E>> for Error {
    fn from(e: broadcast::error::SendError<E>) -> Self {
        Error {
            error: format!("{}", e.to_string()),
        }
    }
}

impl From<&str> for Error {
    fn from(e: &str) -> Self {
        Error {
            error: format!("{:?}", e),
        }
    }
}

impl From<String> for Error {
    fn from(e: String) -> Self {
        Error {
            error: format!("{:?}", e),
        }
    }
}

impl From<Canceled> for Error {
    fn from(e: Canceled) -> Self {
        Error {
            error: format!("{}", e),
        }
    }
}

impl<T> From<SendError<T>> for Error {
    fn from(e: SendError<T>) -> Self {
        Error {
            error: format!("{}", e),
        }
    }
}


