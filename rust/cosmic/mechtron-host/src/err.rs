use wasmer::CompileError;
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
