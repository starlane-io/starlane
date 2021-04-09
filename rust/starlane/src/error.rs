use std::fmt;
use std::fmt::Formatter;
use crate::message::ProtoGram;
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

impl From<SendError<ProtoGram>> for Error{
    fn from(e: SendError<ProtoGram>) -> Self {
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