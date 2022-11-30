use std::convert::Infallible;
use std::io;
use std::string::FromUtf8Error;
use std::sync::{MutexGuard, PoisonError};
use cosmic_space::err::SpaceErr;
use crate::cli::CliConfig;

#[derive(Debug)]
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


impl From<reqwest::Error> for CliErr {
    fn from(e: reqwest::Error) -> Self {
        CliErr::new(e.to_string())
    }
}

impl From<PoisonError<std::sync::MutexGuard<'_, CliConfig>>> for CliErr {
    fn from(e: PoisonError<MutexGuard<'_, CliConfig>>) -> Self {
        CliErr::new(e.to_string())
    }
}

impl From<SpaceErr> for CliErr {
    fn from(e: SpaceErr) -> Self {
        CliErr::new(e.to_string())
    }
}

impl Into<SpaceErr> for CliErr {
    fn into(self) -> SpaceErr {
        SpaceErr::new(500, self.message )
    }
}