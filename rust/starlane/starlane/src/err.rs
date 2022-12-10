use std::num::ParseIntError;

#[cfg(feature="keycloak")]
use keycloak::KeycloakError;

use cosmic_hyperspace::err::ErrKind;

#[cfg(feature = "postgres")]
use cosmic_registry_postgres::err::PostErr;

#[cfg(feature="postgres")]
#[cfg(not(feature="keycloak"))]
pub trait StarlaneErr: PostErr {}

#[cfg(not(feature = "postgres"))]
pub trait StarlaneErr {}

#[cfg(feature="keycloak")]
#[cfg(not(feature="postgres"))]
pub trait StarlaneErr: From<KeycloakError>+From<ParseIntError>{}

#[cfg(feature="keycloak")]
#[cfg(feature="postgres")]
pub trait StarlaneErr: PostErr+From<KeycloakError>+From<ParseIntError>{}

#[derive(Debug, Clone)]
pub struct StarErr {
    pub kind: ErrKind,
    pub message: String,
}

impl StarlaneErr for StarErr {}

pub mod convert {
    use crate::err::StarErr as Err;
    use ascii::FromAsciiError;
    use bincode::ErrorKind;
    use cosmic_hyperspace::err::{ErrKind, HyperErr};
    //    use cosmic_registry_postgres::err::PostErr;
    #[cfg(feature = "postgres")]
    use cosmic_registry_postgres::err::PostErr;
    #[cfg(feature = "postgres")]
    use sqlx::Error;

    use cosmic_space::err::SpaceErr;
    use mechtron_host::err::{DefaultHostErr, HostErr};

    use std::io;
    use std::num::ParseIntError;
    use std::str::Utf8Error;
    use std::string::FromUtf8Error;
    use base64::DecodeError;
    #[cfg(feature = "keycloak")]
    use keycloak::KeycloakError;
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


    impl From<serde_json::Error> for Err {
        fn from(e:serde_json::Error) -> Self {
            Self{
                kind: ErrKind::Default,
                message: e.to_string()
            }
        }
    }

     impl From<alcoholic_jwt::ValidationError> for Err {
        fn from(e:alcoholic_jwt::ValidationError) -> Self {
            Self{
                kind: ErrKind::Default,
                message: e.to_string()
            }
        }
    }


    impl From<DecodeError> for Err {
        fn from(e:DecodeError) -> Self {
            Self{
                kind: ErrKind::Default,
                message: e.to_string()
            }
        }
    }
    #[cfg(feature="keycloak")]
    impl From<KeycloakError> for Err {
        fn from(e: KeycloakError) -> Self {
            Self{
                kind: ErrKind::Default,
                message: e.to_string()
            }
        }
    }

    #[cfg(feature="keycloak")]
    impl From<ParseIntError> for Err {
        fn from(e: ParseIntError) -> Self {
            Self{
                kind: ErrKind::Default,
                message: e.to_string()
            }
        }
    }


    impl From<strum::ParseError> for Err {
        fn from(e: strum::ParseError) -> Self {
            Self {
                kind: ErrKind::Default,
                message: e.to_string(),
            }
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
    impl From<FromAsciiError<std::string::String>> for Err {
        fn from(e: FromAsciiError<String>) -> Self {
            Self {
                kind: ErrKind::Default,
                message: e.to_string(),
            }
        }
    }

    impl ToString for Err {
        fn to_string(&self) -> String {
            self.message.clone()
        }
    }

    #[cfg(feature = "postgres")]
    impl PostErr for Err {
        fn dupe() -> Self {
            Self {
                kind: ErrKind::Dupe,
                message: "Dupe".to_string(),
            }
        }
    }

    impl From<DefaultHostErr> for Err {
        fn from(e: DefaultHostErr) -> Self {
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

    #[cfg(feature="postgres")]
    impl From<sqlx::Error> for Err {
        fn from(e: sqlx::Error) -> Self {
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
