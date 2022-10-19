use cosmic_hyperspace::err::{ErrKind, HyperErr};
use cosmic_space::err::SpaceErr;
use std::io::{Error, ErrorKind};
use std::string::FromUtf8Error;
use strum::ParseError;

pub trait PostErr: HyperErr + From<sqlx::Error> + From<ParseError> {
    fn dupe() -> Self;
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub struct TestErr {
    pub message: String,
    pub kind: ErrKind,
}

#[cfg(test)]
impl PostErr for TestErr {
    fn dupe() -> Self {
        Self {
            kind: ErrKind::Dupe,
            message: "Dupe".to_string(),
        }
    }
}

#[cfg(test)]
pub mod convert {
    use crate::err::TestErr as Err;
    use crate::HyperErr;
    use bincode::ErrorKind;
    use cosmic_hyperspace::err::ErrKind;
    use cosmic_space::err::SpaceErr;
    use mechtron_host::err::HostErr;
    use sqlx::Error;
    use std::io;
    use std::str::Utf8Error;
    use std::string::FromUtf8Error;
    use strum::ParseError;
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

        impl From<url::ParseError> for Err {
        fn from(e: url::ParseError) -> Self {
            Self {
                kind: ErrKind::Default,
                message: e.to_string(),
            }
        }
    }
    impl From<ascii::FromAsciiError<std::string::String>> for Err {
        fn from(e: ascii::FromAsciiError<String>) -> Self {
            Self {
                kind: ErrKind::Default,
                message: e.to_string(),
            }
        }
    }
    impl HyperErr for Err {
        fn to_space_err(&self) -> SpaceErr {
            SpaceErr::server_error(self.to_string())
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

    impl From<()> for Err {
        fn from(_: ()) -> Self {
            Err::new("empty")
        }
    }

    impl From<ParseError> for Err {
        fn from(e: ParseError) -> Self {
            Err::new(e)
        }
    }

    impl From<sqlx::Error> for Err {
        fn from(e: Error) -> Self {
            Err::new(e)
        }
    }

    impl Into<SpaceErr> for Err {
        fn into(self) -> SpaceErr {
            SpaceErr::server_error(self.to_string())
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

    impl From<SpaceErr> for Err {
        fn from(err: SpaceErr) -> Self {
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
        fn to_space_err(self) -> SpaceErr {
            SpaceErr::server_error(self.to_string())
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
