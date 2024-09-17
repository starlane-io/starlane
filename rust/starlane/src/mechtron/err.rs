use std::fmt::{Debug, Formatter};
use std::str::Utf8Error;
use std::string::FromUtf8Error;
use bincode::ErrorKind;
use tokio::sync::oneshot::error::RecvError;
use wasmer::{CompileError, ExportError, InstantiationError, RuntimeError};
use starlane_space::err::SpaceErr;

pub trait HostErr:
    Debug
    + ToString
    + From<CompileError>
    + From<RuntimeError>
    + From<String>
    + From<&'static str>
    + From<Box<bincode::ErrorKind>>
    + From<ExportError>
    + From<tokio::sync::oneshot::error::RecvError>
    + From<Utf8Error>
    + From<FromUtf8Error>
    + From<InstantiationError>
    + From<SpaceErr>
    + Into<SpaceErr>
{
    fn to_space_err(self) -> SpaceErr;
}

pub struct DefaultHostErr {
    pub message: String
}

impl ToString for DefaultHostErr {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}

impl Debug for DefaultHostErr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl From<CompileError> for DefaultHostErr {
    fn from(value: CompileError) -> Self {
        todo!()
    }
}

impl From<RuntimeError> for DefaultHostErr {
    fn from(value: RuntimeError) -> Self {
        todo!()
    }
}

impl From<String> for DefaultHostErr {
    fn from(value: String) -> Self {
        todo!()
    }
}

impl From<&'static str> for DefaultHostErr {
    fn from(value: &'static str) -> Self {
        todo!()
    }
}

impl From<Box<ErrorKind>> for DefaultHostErr {
    fn from(value: Box<ErrorKind>) -> Self {
        todo!()
    }
}

impl From<ExportError> for DefaultHostErr {
    fn from(value: ExportError) -> Self {
        todo!()
    }
}

impl From<RecvError> for DefaultHostErr {
    fn from(value: RecvError) -> Self {
        todo!()
    }
}

impl From<Utf8Error> for DefaultHostErr {
    fn from(value: Utf8Error) -> Self {
        todo!()
    }
}

impl From<FromUtf8Error> for DefaultHostErr {
    fn from(value: FromUtf8Error) -> Self {
        todo!()
    }
}

impl From<InstantiationError> for DefaultHostErr {
    fn from(value: InstantiationError) -> Self {
        todo!()
    }
}

impl From<SpaceErr> for DefaultHostErr {
    fn from(value: SpaceErr) -> Self {
        todo!()
    }
}

impl Into<SpaceErr> for DefaultHostErr {
    fn into(self) -> SpaceErr {
        todo!()
    }
}

impl HostErr for DefaultHostErr {
    fn to_space_err(self) -> SpaceErr {
        SpaceErr::server_error(self.to_string())
    }
}