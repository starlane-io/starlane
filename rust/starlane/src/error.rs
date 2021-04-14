use std::fmt;
use std::fmt::Formatter;
use crate::frame::ProtoFrame;
use tokio::sync::mpsc::error::SendError;

#[derive(Debug, Clone)]
pub struct Error{
    pub error: String
}

impl fmt::Display for Error{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}",self.error)
    }
}

/*impl From<SendError<ProtoFrame>> for Error{
    fn from(e: SendError<ProtoFrame>) -> Self {
        Error{
            error: format!("{}",e.to_string())
        }
    }
}

 */

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

impl <T> From<SendError<T>> for Error{
    fn from(e: SendError<T>) -> Self {
        Error{
            error: format!("{}",e)
        }
    }
}