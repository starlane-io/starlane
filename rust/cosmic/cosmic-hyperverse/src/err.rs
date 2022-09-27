use cosmic_universe::err::UniErr;
use wasmer::{CompileError, ExportError, InstantiationError, RuntimeError};
use std::str::Utf8Error;
use std::io;
use std::io::Error;
use mechtron_host::err::HostErr;
use tokio::time::error::Elapsed;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;
use std::string::FromUtf8Error;
use crate::HyperErr;

#[derive(Debug, Clone)]
pub struct CosmicErr {
    pub message: String,
}

impl CosmicErr {
    pub fn new<S: ToString>(message: S) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl ToString for CosmicErr {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}

impl Into<UniErr> for CosmicErr {
    fn into(self) -> UniErr {
        UniErr::from_500(self.to_string())
    }
}

impl From<oneshot::error::RecvError> for CosmicErr {
    fn from(err: RecvError) -> Self {
        CosmicErr {
            message: err.to_string(),
        }
    }
}

impl From<Elapsed> for CosmicErr {
    fn from(err: Elapsed) -> Self {
        CosmicErr {
            message: err.to_string(),
        }
    }
}

impl From<String> for CosmicErr {
    fn from(err: String) -> Self {
        CosmicErr { message: err }
    }
}

impl From<&'static str> for CosmicErr {
    fn from(err: &'static str) -> Self {
        CosmicErr {
            message: err.to_string(),
        }
    }
}

impl From<UniErr> for CosmicErr {
    fn from(err: UniErr) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

impl From<io::Error> for CosmicErr {
    fn from(err: Error) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

impl From<acid_store::Error> for CosmicErr {
    fn from(e: acid_store::Error) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

impl From<zip::result::ZipError> for CosmicErr {
    fn from(a: zip::result::ZipError) -> Self {
        Self {
            message: a.to_string(),
        }
    }
}

impl From<Box<bincode::ErrorKind>> for CosmicErr {
    fn from(e: Box<bincode::ErrorKind>) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

impl HostErr for CosmicErr {
    fn to_uni_err(self) -> UniErr {
        UniErr::from_500(self.to_string())
    }
}

impl From<CompileError> for CosmicErr {
    fn from(e: CompileError) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

impl From<RuntimeError> for CosmicErr {
    fn from(e: RuntimeError) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

impl From<ExportError> for CosmicErr {
    fn from(e: ExportError) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

impl From<Utf8Error> for CosmicErr {
    fn from(e: Utf8Error) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

impl From<FromUtf8Error> for CosmicErr {
    fn from(e: FromUtf8Error) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

impl From<InstantiationError> for CosmicErr {
    fn from(e: InstantiationError) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

impl HyperErr for CosmicErr {
    fn to_uni_err(&self) -> UniErr {
        UniErr::from_500(self.to_string())
    }

    fn new<S>(message: S) -> Self
    where
        S: ToString,
    {
        Self {
            message: message.to_string(),
        }
    }

    fn status_msg<S>(status: u16, message: S) -> Self
    where
        S: ToString,
    {
        Self {
            message: message.to_string(),
        }
    }

    fn status(&self) -> u16 {
        500u16
    }
}
