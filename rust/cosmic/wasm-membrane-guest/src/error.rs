use alloc::format;
use alloc::string::{FromUtf8Error, String, ToString};
use core::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub struct Error {
    pub error: String,
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

/*
impl<T> From<PoisonError<T>> for Error {
    fn from(e: PoisonError<T>) -> Self {
        Error {
            error: format!("{:?}", e),
        }
    }
}

 */

impl From<FromUtf8Error> for Error {
    fn from(e: FromUtf8Error) -> Self {
        Error {
            error: format!("{:?}", e),
        }
    }
}


impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{}",self.error))
    }
}
