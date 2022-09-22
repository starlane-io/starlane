use wasmer::CompileError;
use wasm_membrane_host::error::Error;

pub struct HostErr{
  pub message: String
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
