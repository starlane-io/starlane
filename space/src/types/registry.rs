use std::sync::Arc;
use crate::types::registry::err::RegErr;
use async_trait::async_trait;
use crate::selector::SpecificSelector;

pub type Registry = Arc<dyn TypeRegistry>;



/// A Registry component interface for accessing Metadata about types including: `BindConfig`, `defined subtypes`
#[async_trait]
pub trait TypeRegistry: Send + Sync {
   async fn select_specific<'a>(&'a self, selector: &'a SpecificSelector) -> Result<Cursor,RegErr>;
}


pub struct Cursor {
}

pub struct RegistryWrapper {
    registry: Registry,
}

impl RegistryWrapper {
    pub fn new(registry: Registry) -> Self {
        Self { registry }
    }
}







pub mod err {
    use crate::point::Point;
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum RegErr {
        #[error("duplicate error")]
        Dupe,

        #[error("expected parent for point `{0}'")]
        ExpectedParent(Point),
        #[error("Registry does not handle GetOp::State operations")]
        NoGetOpStateOperations,
        #[error("Database Setup Failed")]
        RegistrySetupFailed,
        #[error("{0}")]
        Msg(String),
        #[error("database has scorch guard enabled.  To change this: 'INSERT INTO reset_mode VALUES ('Scorch')'"
        )]
        NoScorch,
        #[error("expected an embedded postgres registry but received configuration for a remote postgres registry"
        )]
        ExpectedEmbeddedRegistry,
    }

   impl From<&str> for RegErr {
        fn from(err: &str) -> Self {
            Self::Msg(err.to_string())
        }
    }

    impl From<&String> for RegErr {
        fn from(err: &String) -> Self {
            Self::Msg(err.to_string())
        }
    }

    impl RegErr {
        pub fn dupe() -> Self {
            Self::Dupe
        }


        pub fn expected_parent(point: &Point) -> Self {
            Self::ExpectedParent(point.clone())
        }

        pub fn msg<M>(msg: M) -> RegErr
        where
            M: ToString,
        {
            Self::Msg(msg.to_string())
        }
    }
}