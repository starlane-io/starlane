use wasmer::{CompileError, RuntimeError};
use wasm_membrane_host::error::Error;

#[derive(Debug)]
pub struct HostErr{
  pub message: String
}

impl ToString for HostErr {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}



impl From<CompileError> for HostErr {
    fn from(e: CompileError) -> Self {
        HostErr {
            message: e.to_string()
        }
    }
}

impl From<wasm_membrane_host::error::Error> for HostErr {
    fn from(e: Error) -> Self {
                HostErr {
            message: e.to_string()
        }
    }
}
impl From<Box<bincode::ErrorKind>> for HostErr {
    fn from(e: Box<bincode::ErrorKind>) -> Self {
        HostErr {
            message: e.to_string()
        }
    }
}

impl From<RuntimeError> for HostErr {
    fn from(e: RuntimeError) -> Self {
        HostErr {
            message: e.to_string()
        }
    }
}

impl From<&str> for HostErr {
    fn from(e: &str) -> Self {
        HostErr {
            message: e.to_string()
        }
    }
}
