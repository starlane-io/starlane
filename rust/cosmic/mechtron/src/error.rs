use bincode::ErrorKind;
use std::sync::{PoisonError, RwLockReadGuard, Arc, RwLockWriteGuard};
use std::collections::HashMap;
use crate::{MechtronFactory, MechtronWrapper};

pub struct Error {
    pub message: String
}

impl ToString for Error {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}

impl From<&str> for Error {
    fn from(error: &str) -> Self {
        Self {
            message: error.to_string()
        }
    }
}

impl From<String> for Error {
    fn from(error: String) -> Self {
        Self {
            message: error
        }
    }
}


impl From<mesh_portal::error::MsgErr> for Error {
    fn from(error: mesh_portal::error::MsgErr) -> Self {
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

impl <T> From<PoisonError<T>> for Error {
    fn from(_: PoisonError<T>) -> Self {
        Self {
            message: "Poison error".to_string()
        }
    }
}

/*
impl From<PoisonError<RwLockWriteGuard<'_, HashMap<String, mesh_portal::version::latest::id::Address>>>> for Error {
    fn from(_: PoisonError<RwLockWriteGuard<'_, HashMap<String, Address>>>) -> Self {
        Self {
            message: "Poison error".to_string()
        }
    }
}


impl From<PoisonError<RwLockWriteGuard<'_, HashMap<mesh_portal::version::latest::id::Address, Arc<MechtronWrapper>>>>> for Error {
    fn from(_: PoisonError<RwLockWriteGuard<'_, HashMap<Address, Arc<MechtronWrapper>>>>) -> Self {
        todo!()
    }
}

 */