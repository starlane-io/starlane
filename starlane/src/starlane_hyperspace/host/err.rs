use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use thiserror::Error;

#[derive(Clone, Debug,Error)]
pub struct HostErr {
    message: String,
}


impl Display for HostErr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}

impl From<Infallible> for HostErr {
    fn from(value: Infallible) -> Self {
        HostErr::new(value.to_string())
    }
}

/*
impl ToString for Err {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}

 */

/*
impl<T> From<T> for HostErr
where
    T: ToString,
{
    fn from(value: T) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

 */

impl HostErr {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

/*
impl From<String> for Err {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<WasiStateCreationError> for Err {
    fn from(value: WasiStateCreationError) -> Self {
        Err( )
    }
}
impl From<WasiRuntimeError> for Err {
    fn from(value: WasiRuntimeError) -> Self {
       Err {
           message: value.to_string()
       }
    }
}
impl From<WasiStateCreationError> for Err {
    fn from(value: WasiStateCreationError) -> Self {
        Self {
            message: value.to_string()
        }
    }
}

impl From<io::Error> for Err {
    fn from(err: io::Error) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

impl ToString for Err {
    fn to_string(&self) -> String {
        todo!()
    }
}


 */
