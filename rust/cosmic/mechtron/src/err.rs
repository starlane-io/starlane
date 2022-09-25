use bincode::ErrorKind;
use cosmic_universe::err::UniErr;
use wasm_membrane_guest::error::Error;

pub trait MechErr: ToString+From<Box<bincode::ErrorKind>>+From<wasm_membrane_guest::error::Error> {
    fn to_uni_err(self) -> UniErr;
}

#[derive(Clone)]
pub struct GuestErr  {
    pub message: String
}

impl ToString for GuestErr {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}

impl MechErr for GuestErr {
    fn to_uni_err(self) -> UniErr {
        UniErr::from_500(self.to_string())
    }
}

impl From<Box<bincode::ErrorKind>> for GuestErr {
    fn from(e: Box<ErrorKind>) -> Self {
        Self {
            message: e.to_string()
        }
    }
}

impl From<wasm_membrane_guest::error::Error> for GuestErr {
    fn from(e: Error) -> Self {
        Self {
            message: e.to_string()
        }
    }
}
