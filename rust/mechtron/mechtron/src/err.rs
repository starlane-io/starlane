use alloc::string::FromUtf8Error;
use bincode::ErrorKind;
use core::fmt::{Display, Formatter};
use cosmic_universe::err::{CoreReflector, UniErr};
use cosmic_universe::substance::Substance;
use cosmic_universe::wave::core::http2::StatusCode;
use cosmic_universe::wave::core::ReflectedCore;

pub trait MechErr:
    CoreReflector
    + ToString
    + From<Box<bincode::ErrorKind>>
    + From<MembraneErr>
    + From<UniErr>
    + From<String>
    + From<&'static str>
    + From<GuestErr>
{
    fn to_uni_err(self) -> UniErr;
}

#[derive(Debug, Clone)]
pub struct GuestErr {
    pub message: String,
}

impl ToString for GuestErr {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}

impl From<MembraneErr> for GuestErr {
    fn from(e: MembraneErr) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

impl From<UniErr> for GuestErr {
    fn from(err: UniErr) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

impl CoreReflector for GuestErr {
    fn as_reflected_core(self) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(500u16).unwrap(),
            body: Substance::Text(self.message),
        }
    }
}

impl From<String> for GuestErr {
    fn from(s: String) -> Self {
        Self { message: s }
    }
}

impl From<&str> for GuestErr {
    fn from(s: &str) -> Self {
        Self {
            message: s.to_string(),
        }
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
            message: e.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MembraneErr {
    pub error: String,
}

impl From<&str> for MembraneErr {
    fn from(e: &str) -> Self {
        MembraneErr {
            error: format!("{:?}", e),
        }
    }
}

impl From<String> for MembraneErr {
    fn from(e: String) -> Self {
        MembraneErr {
            error: format!("{:?}", e),
        }
    }
}

/*
impl<T> From<PoisonError<T>> for Error {
    fn from(e: PoisonError<T>) -> Self {
        Error {
            error: format!("{:?}", e),
        }
    }
}

 */

impl From<FromUtf8Error> for MembraneErr {
    fn from(e: FromUtf8Error) -> Self {
        MembraneErr {
            error: format!("{:?}", e),
        }
    }
}

impl Display for MembraneErr {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{}", self.error))
    }
}
