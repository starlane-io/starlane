use bincode::ErrorKind;

pub struct Error {
    pub message: String
}

impl From<mesh_portal_serde::error::Error> for Error {
    fn from(error: mesh_portal_serde::error::Error) -> Self {
        Self {
            message: error.to_string()
        }
    }
}

impl From<wasm_membrane_guest::error::Error> for Error {
    fn from(error: wasm_membrane_guest::error::Error) -> Self {
        Self {
            message: error.to_string()
        }
    }
}

impl From<Box<bincode::ErrorKind>> for Error {
    fn from(error: Box<ErrorKind>) -> Self {
        Self {
            message: error.to_string()
        }
    }
}
