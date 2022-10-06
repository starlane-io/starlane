use cosmic_universe::err::UniErr;
use cosmic_universe::substance::Substance;
use cosmic_universe::wave::core::http2::StatusCode;
use cosmic_universe::wave::core::ReflectedCore;
use mechtron_host::err::HostErr;
use std::fmt::Debug;
use std::io;
use std::io::Error;
use std::str::Utf8Error;
use std::string::FromUtf8Error;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;
use tokio::time::error::Elapsed;
use wasmer::{CompileError, ExportError, InstantiationError, RuntimeError};

#[derive(Debug, Clone,Eq, PartialEq)]
pub enum ErrKind {
    Default,
    Dupe,
    Status(u16),
}

#[derive(Debug, Clone)]
pub struct CosmicErr {
    pub kind: ErrKind,
    pub message: String,
}

pub trait HyperErr:
    Sized
    + Debug
    + Send
    + Sync
    + ToString
    + Clone
    + HostErr
    + Into<UniErr>
    + From<UniErr>
    + From<String>
    + From<&'static str>
    + From<tokio::sync::oneshot::error::RecvError>
    + From<std::io::Error>
    + From<zip::result::ZipError>
    + From<Box<bincode::ErrorKind>>
    + From<acid_store::Error>
    + From<UniErr>
    + Into<UniErr>
    + From<()>
{
    fn to_uni_err(&self) -> UniErr;

    fn new<S>(message: S) -> Self
    where
        S: ToString;

    fn status_msg<S>(status: u16, message: S) -> Self
    where
        S: ToString;

    fn not_found() -> Self {
        Self::not_found_msg("Not Found")
    }

    fn not_found_msg<S>(message: S) -> Self
    where
        S: ToString,
    {
        Self::status_msg(404, message)
    }

    fn status(&self) -> u16;

    fn as_reflected_core(&self) -> ReflectedCore {
        let mut core = ReflectedCore::new();
        core.status =
            StatusCode::from_u16(self.status()).unwrap_or(StatusCode::from_u16(500u16).unwrap());
        core.body = Substance::Empty;
        core
    }

    fn kind(&self) -> ErrKind;

    fn with_kind<S>(kind: ErrKind, msg: S) -> Self
    where
        S: ToString;
}

pub mod convert {
    use crate::err::{CosmicErr as Err, ErrKind};
    use crate::HyperErr;
    use bincode::ErrorKind;
    use cosmic_universe::err::UniErr;
    use mechtron_host::err::HostErr;
    use std::io;
    use std::str::Utf8Error;
    use std::string::FromUtf8Error;
    use tokio::sync::oneshot;
    use tokio::time::error::Elapsed;
    use wasmer::{CompileError, ExportError, InstantiationError, RuntimeError};

    impl Err {
        pub fn new<S: ToString>(message: S) -> Self {
            Self {
                message: message.to_string(),
                kind: ErrKind::Default,
            }
        }
    }

    impl ToString for Err {
        fn to_string(&self) -> String {
            self.message.clone()
        }
    }

    impl From<()> for Err {
        fn from(_: ()) -> Self {
            Err::new("Empty")
        }
    }

    impl HyperErr for Err {
        fn to_uni_err(&self) -> UniErr {
            UniErr::from_500(self.to_string())
        }

        fn new<S>(message: S) -> Self
        where
            S: ToString,
        {
            Err::new(message)
        }

        fn status_msg<S>(status: u16, message: S) -> Self
        where
            S: ToString,
        {
            Err::new(message)
        }

        fn status(&self) -> u16 {
            if let ErrKind::Status(code) = self.kind {
                code
            } else {
                500u16
            }
        }

        fn kind(&self) -> ErrKind {
            self.kind.clone()
        }

        fn with_kind<S>(kind: ErrKind, msg: S) -> Self
        where
            S: ToString,
        {
            Err {
                kind,
                message: msg.to_string(),
            }
        }
    }
    impl Into<UniErr> for Err {
        fn into(self) -> UniErr {
            UniErr::from_500(self.to_string())
        }
    }

    impl From<oneshot::error::RecvError> for Err {
        fn from(err: oneshot::error::RecvError) -> Self {
            Err::new(err)
        }
    }

    impl From<Elapsed> for Err {
        fn from(err: Elapsed) -> Self {
            Err::new(err)
        }
    }

    impl From<String> for Err {
        fn from(err: String) -> Self {
            Err::new(err)
        }
    }

    impl From<&'static str> for Err {
        fn from(err: &'static str) -> Self {
            Err::new(err)
        }
    }

    impl From<UniErr> for Err {
        fn from(err: UniErr) -> Self {
            Err::new(err)
        }
    }

    impl From<io::Error> for Err {
        fn from(err: io::Error) -> Self {
            Err::new(err)
        }
    }

    impl From<acid_store::Error> for Err {
        fn from(e: acid_store::Error) -> Self {
            Err::new(e)
        }
    }

    impl From<zip::result::ZipError> for Err {
        fn from(a: zip::result::ZipError) -> Self {
            Err::new(a)
        }
    }

    impl From<Box<bincode::ErrorKind>> for Err {
        fn from(e: Box<bincode::ErrorKind>) -> Self {
            Err::new(e)
        }
    }


    impl From<ExportError> for Err {
        fn from(e: ExportError) -> Self {
            Err::new(e)
        }
    }

    impl From<Utf8Error> for Err {
        fn from(e: Utf8Error) -> Self {
            Err::new(e)
        }
    }

    impl From<FromUtf8Error> for Err {
        fn from(e: FromUtf8Error) -> Self {
            Err::new(e)
        }
    }

    impl From<InstantiationError> for Err {
        fn from(_: InstantiationError) -> Self {
            todo!()
        }
    }

    impl HostErr for Err {
        fn to_uni_err(self) -> UniErr {
            UniErr::from_500(self.to_string())
        }
    }

    impl From<CompileError> for Err {
        fn from(e: CompileError) -> Self {
            Err::new(e)
        }
    }

    impl From<RuntimeError> for Err {
        fn from(e: RuntimeError) -> Self {
            Err::new(e)
        }
    }


}
