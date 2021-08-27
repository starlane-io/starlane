use wasmer::{CompileError, RuntimeError, ExportError, InstantiationError};
use std::fmt::{Formatter, Debug};
use core::fmt;
use std::string::FromUtf8Error;
use std::sync::PoisonError;
use std::io;

#[derive(Debug, Clone)]
pub struct Error{
    pub error: String
}


impl fmt::Display for Error{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "nucleus error: {:?}",self)
    }
}


impl From<Box<dyn Debug>> for Error{
    fn from(e: Box<dyn Debug>) -> Self {
        Error{
            error: format!("{:?}",e)
        }
    }
}

impl From<&dyn Debug> for Error{
    fn from(e: &dyn Debug) -> Self {
        Error{
            error: format!("{:?}",e)
        }
    }
}

impl From<io::Error> for Error{
    fn from(e: io::Error) -> Self {
        Error{
            error: format!("{:?}",e)
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

impl<T> From<PoisonError<T>> for Error {
    fn from(e: PoisonError<T>) -> Self {
        Error {
            error: format!("{:?}", e)
        }
    }
}

impl From<FromUtf8Error> for Error {

    fn from(e:FromUtf8Error) -> Self {
        Error {
            error: format!("{:?}", e)
        }
    }
}

impl From<RuntimeError> for Error{
    fn from(e: RuntimeError) -> Self {
        Error{
            error: format!("{:?}",e)
        }
    }
}

impl From<ExportError> for Error{
    fn from(e: ExportError) -> Self {
        Error{
            error: format!("{:?}",e)
        }
    }
}

impl From<CompileError> for Error{
    fn from(e: CompileError) -> Self {
        Error{
            error: format!("{:?}",e)
        }
    }
}

impl From<InstantiationError> for Error{
    fn from(e: InstantiationError) -> Self {
        Error{
            error: format!("{:?}",e)
        }
    }
}