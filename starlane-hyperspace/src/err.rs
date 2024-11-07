use std::convert::Infallible;
use std::io::Error;
use std::sync::Arc;
use strum_macros::Display;
use thiserror::Error;
use tokio::io;
use tokio::sync::oneshot::error::RecvError;

#[cfg(feature = "postgres")]
use crate::registry::err::RegErr;
use starlane::space::err::{CoreReflector, HyperSpatialError, SpaceErr, SpatialError};
use starlane::space::kind::Kind;
use starlane::space::point::Point;
use starlane::space::substance::{Substance, SubstanceKind};
use starlane::space::wave::core::http2::StatusCode;
use starlane::space::wave::core::ReflectedCore;


pub type HyperErr2 = anyhow::Error;
pub use anyhow::anyhow as err;
use crate::star::StarErr;

#[derive(Error, Debug)]
pub enum HypErr {
    #[error(transparent)]
    SpaceErr(#[from] SpaceErr),
    #[error(transparent)]
    RegErr(#[from] RegErr),
    #[error(transparent)]
    StarErr(#[from] StarErr),
    #[error(transparent)]
    Anyhow(#[from] Arc<anyhow::Error>),
    #[error("{0}")]
    String(String),
    #[error("{0}")]
    Iniff(#[from] Infallible),
    #[error("{0}")]
    StripPrefix(#[from] std::path::StripPrefixError),
    #[error("{0}")]
    OneshotRecvErr(#[from] oneshot::RecvError),
    #[error("{0}")]
    Io(#[from] Arc<tokio::io::Error>),
}

impl SpatialError for HypErr {}

impl HyperSpatialError for HypErr {}

impl From<anyhow::Error> for HypErr {
    fn from(value: anyhow::Error) -> Self {
        HypErr::Anyhow(Arc::new(value))
    }
}

impl From<tokio::io::Error> for HypErr {
    fn from(value: Error) -> Self {
        HypErr::Io(Arc::new(value))
    }
}

impl CoreReflector for HypErr {
    fn as_reflected_core(self) -> ReflectedCore {
        match self {
            HypErr::SpaceErr(err) => err.as_reflected_core(),
            m => {
                let err = SpaceErr::Msg(m.to_string());
                ReflectedCore {
                    headers: Default::default(),
                    status: StatusCode::from_u16(500u16).unwrap(),
                    body: err.to_substance()
                }
            }
        }
    }
}

pub enum PlatformErr {}

impl From<&str> for HypErr {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

/*
#[derive(Debug, Clone,Error)]
pub struct OldStarErr {
    pub kind: ErrKind,
    pub message: String,
}

 */

/*
impl From<ThisErr> for OldStarErr {
    fn from(value: ThisErr) -> Self {
        OldStarErr::new( value.to_string())
    }
}

 */

/*
pub mod convert {
    use starlane_space as starlane;
    use crate::err::OldStarErr;
    use crate::hyperspace::err::{ErrKind, HyperErr};
    use ascii::FromAsciiError;
    use std::io;
    use std::str::Utf8Error;
    use std::string::FromUtf8Error;
    use tokio::sync::oneshot;
    use tokio::time::error::Elapsed;
    use wasmer::{CompileError, ExportError, InstantiationError, RuntimeError};
    use starlane::space::err::SpaceErr;

    impl From<strum::ParseError> for OldStarErr {
        fn from(e: strum::ParseError) -> Self {
            Self {
                kind: ErrKind::Default,
                message: e.to_string(),
            }
        }
    }

    impl From<url::ParseError> for OldStarErr {
        fn from(e: url::ParseError) -> Self {
            Self {
                kind: ErrKind::Default,
                message: e.to_string(),
            }
        }
    }
    impl From<FromAsciiError<std::string::String>> for OldStarErr {
        fn from(e: FromAsciiError<String>) -> Self {
            Self {
                kind: ErrKind::Default,
                message: e.to_string(),
            }
        }
    }

    impl ToString for OldStarErr {
        fn to_string(&self) -> String {
            self.message.clone()
        }
    }

    impl From<()> for OldStarErr {
        fn from(_: ()) -> Self {
            OldStarErr::new("empty")
        }
    }

    impl From<sqlx::Error> for OldStarErr {
        fn from(e: sqlx::Error) -> Self {
            OldStarErr::new(e)
        }
    }

    impl Into<SpaceErr> for OldStarErr {
        fn into(self) -> SpaceErr {
            SpaceErr::server_error(self.to_string())
        }
    }

    impl From<oneshot::error::RecvError> for OldStarErr {
        fn from(err: oneshot::error::RecvError) -> Self {
            OldStarErr::new(err)
        }
    }

    impl From<Elapsed> for OldStarErr {
        fn from(err: Elapsed) -> Self {
            OldStarErr::new(err)
        }
    }

    impl From<String> for OldStarErr {
        fn from(err: String) -> Self {
            OldStarErr::new(err)
        }
    }

    impl From<&'static str> for OldStarErr {
        fn from(err: &'static str) -> Self {
            OldStarErr::new(err)
        }
    }

    impl From<SpaceErr> for OldStarErr {
        fn from(err: SpaceErr) -> Self {
            OldStarErr::new(err)
        }
    }

    impl From<io::Error> for OldStarErr {
        fn from(err: io::Error) -> Self {
            OldStarErr::new(err)
        }
    }

    impl From<zip::result::ZipError> for OldStarErr {
        fn from(a: zip::result::ZipError) -> Self {
            OldStarErr::new(a)
        }
    }

    impl From<Box<bincode::ErrorKind>> for OldStarErr {
        fn from(e: Box<bincode::ErrorKind>) -> Self {
            OldStarErr::new(e)
        }
    }

    impl From<ExportError> for OldStarErr {
        fn from(e: ExportError) -> Self {
            OldStarErr::new(e)
        }
    }

    impl From<Utf8Error> for OldStarErr {
        fn from(e: Utf8Error) -> Self {
            OldStarErr::new(e)
        }
    }

    impl From<FromUtf8Error> for OldStarErr {
        fn from(e: FromUtf8Error) -> Self {
            OldStarErr::new(e)
        }
    }

    impl From<InstantiationError> for OldStarErr {
        fn from(_: InstantiationError) -> Self {
            todo!()
        }
    }

    impl From<CompileError> for OldStarErr {
        fn from(e: CompileError) -> Self {
            OldStarErr::new(e)
        }
    }

    impl From<RuntimeError> for OldStarErr {
        fn from(e: RuntimeError) -> Self {
            OldStarErr::new(e)
        }
    }
}

 */


