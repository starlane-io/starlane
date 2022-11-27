use std::convert::Infallible;
use std::io;
use std::string::FromUtf8Error;

pub struct CliErr{
    pub message: String
}

impl CliErr {
    pub fn new<S:ToString>(message: S) -> Self {
        Self {
            message: message.to_string()
        }
    }
}

impl From<Infallible> for CliErr {
    fn from(e: Infallible) -> Self {
        CliErr::new(e.to_string())
    }
}

impl From<io::Error> for CliErr {
    fn from(e: io::Error) -> Self {
        CliErr::new(e.to_string())
    }
}

impl From<FromUtf8Error> for CliErr {
    fn from(e: FromUtf8Error) -> Self {
        CliErr::new(e.to_string())
    }
}


impl From<serde_json::Error> for CliErr {

    fn from(e: serde_json::Error) -> Self {
        CliErr::new(e.to_string())
    }
}


impl From<&str> for CliErr {

    fn from(e: &str) -> Self {
        CliErr::new(e.to_string())
    }
}
