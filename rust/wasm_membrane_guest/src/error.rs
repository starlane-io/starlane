use std::sync::PoisonError;
use std::string::FromUtf8Error;

#[derive(Debug, Clone)]
pub struct Error{
    pub error: String
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