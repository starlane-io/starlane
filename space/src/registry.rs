use serde_derive::{Deserialize, Serialize};
use err::RegErr;
use crate::{Access, AccessGrant, Delete, IndexedAccessGrant, Kind, ParticleRecord, Point, Properties, Query, QueryResult, Select, Selector, SetProperties, SetRegistry, Status, Strategy, SubstanceList};

#[derive(Clone,Debug,Serialize, Deserialize, Eq, PartialEq)]
pub struct Registration {
    pub point: Point,
    pub kind: Kind,
    pub registry: SetRegistry,
    pub properties: SetProperties,
    pub owner: Point,
    pub strategy: Strategy,
    pub status: Status,
}

#[derive(Clone,Debug,Serialize, Deserialize, Eq, PartialEq, strum_macros::Display)]
pub enum RegistryRequest {
    Scorch,
    Register(Registration),
    AssignStar {
        point: Point,
        star: Point,
    },
    AssignHost {
        point: Point,
        host: Point,
    },
    SetStatus {
        point: Point,
        status: Status,
    },
    SetProperties {
        point: Point,
        properties: SetProperties,
    },
    Sequence(Point),
    GetProperties(Point),
    Record(Point),
    Query {
        point: Point,
        query: Query,
    },
    Delete(Delete),
    Select(Select),

    Grant(AccessGrant),
    Access {
        to: Point,
        on: Point,
    },
    Chown {
        on: Selector,
        owner: Point,
        by: Point,
    },
    ListAccess {
        to: Option<Point>,
        on: Selector,
    },
    RemoveAccess {
        id: i32,
        to: Point,
    },
}

#[derive(Clone,Debug,Serialize, Deserialize, Eq, PartialEq, strum_macros::Display)]
pub enum RegistryResponse {
    Err(RegErr),
    Scorch,
    Register,
    AssignStar,
    AssignHost,
    SetStatus,
    SetProperties,
    Sequence(u64),
    GetProperties(Properties),
    Record(ParticleRecord),
    Query(QueryResult),
    Delete(SubstanceList),
    Select(SubstanceList),
    Grant,
    Access(Access),
    Chown,
    ListAccess(Vec<IndexedAccessGrant>),
    RemoveAccess,
}

impl TryInto<Result<(), RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<(), RegErr>, Self::Error> {
        match self {
            RegistryResponse::Err(err) => Ok(Err(err)),
            RegistryResponse::Scorch => Ok(Ok(())),
            RegistryResponse::Register => Ok(Ok(())),
            RegistryResponse::AssignStar => Ok(Ok(())),
            RegistryResponse::AssignHost => Ok(Ok(())),
            RegistryResponse::SetStatus => Ok(Ok(())),
            RegistryResponse::SetProperties => Ok(Ok(())),
            RegistryResponse::Grant => Ok(Ok(())),
            RegistryResponse::Chown => Ok(Ok(())),
            RegistryResponse::RemoveAccess => Ok(Ok(())),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<(), RegErr>",
            )),
        }
    }
}

impl TryInto<Result<u64, RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<u64, RegErr>, Self::Error> {
        match self {
            RegistryResponse::Err(err) => Ok(Err(err)),
            RegistryResponse::Sequence(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<u64, RegErr>",
            )),
        }
    }
}

impl TryInto<Result<Properties, RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<Properties, RegErr>, Self::Error> {
        match self {
            RegistryResponse::Err(err) => Ok(Err(err)),
            RegistryResponse::GetProperties(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<Properties, RegErr>",
            )),
        }
    }
}

impl TryInto<Result<ParticleRecord, RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<ParticleRecord, RegErr>, Self::Error> {
        match self {
            RegistryResponse::Err(err) => Ok(Err(err)),
            RegistryResponse::Record(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<ParticleRecord, RegErr>",
            )),
        }
    }
}

impl TryInto<Result<QueryResult, RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<QueryResult, RegErr>, Self::Error> {
        match self {
            RegistryResponse::Err(err) => Ok(Err(err)),
            RegistryResponse::Query(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<QueryResult, RegErr>",
            )),
        }
    }
}

impl TryInto<Result<SubstanceList, RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<SubstanceList, RegErr>, Self::Error> {
        match self {
            RegistryResponse::Err(err) => Ok(Err(err)),
            RegistryResponse::Delete(value) => Ok(Ok(value)),
            RegistryResponse::Select(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<SubstanceList, RegErr>",
            )),
        }
    }
}

impl TryInto<Result<Access, RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<Access, RegErr>, Self::Error> {
        match self {
            RegistryResponse::Err(err) => Ok(Err(err)),
            RegistryResponse::Access(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<Access, RegErr>",
            )),
        }
    }
}

impl TryInto<Result<Vec<IndexedAccessGrant>, RegErr>> for RegistryResponse {
    type Error = RegErr;

    fn try_into(self) -> Result<Result<Vec<IndexedAccessGrant>, RegErr>, Self::Error> {
        match self {
            RegistryResponse::Err(err) => Ok(Err(err)),
            RegistryResponse::ListAccess(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<Vec<IndexedAccessGrant>, RegErr>",
            )),
        }
    }
}

pub mod err {

    use serde_derive::{Deserialize, Serialize};
    use starlane_space::err::{HyperSpatialError, ParseErrs0, SpaceErr, SpatialError};
    use starlane_space::point::Point;
    use std::sync::Arc;
    use thiserror::Error;


    #[derive(Error,Debug,Clone,Serialize, Deserialize, Eq, PartialEq )]
    pub enum RegErr {
        #[error(transparent)]
        Parse(#[from] ParseErrs0),
        #[error("duplicate error")]
        Dupe,
        #[error("particle not found: '{0}'")]
        NotFound(Point),

        #[error(transparent)]
        SpaceErr(#[from] SpaceErr),

        #[error("expected parent for point `{0}'")]
        ExpectedParent(Point),
        #[error("Registry does not handle GetOp::State operations")]
        NoGetOpStateOperations,
        #[error("Database Setup Failed")]
        RegistrySetupFailed,
        #[error("Point '{point}' registry error: {message}")]
        Point { point: Point, message: String },
        #[error("{0}")]
        Msg(String),

        #[error("postgres registry db connection pool '{0}' not found")]
        PoolNotFound(String),

        #[error("{0}")]
        IoErr(String),
        #[error("database has scorch guard enabled.  To change this: 'INSERT INTO reset_mode VALUES ('Scorch')'"
        )]
        NoScorch,
        #[error("expected an embedded postgres registry but received configuration for a remote postgres registry"
        )]
        ExpectedEmbeddedRegistry,
    }

    impl From<std::io::Error> for RegErr {
        fn from(value: std::io::Error) -> Self {
            Self::IoErr(value.to_string())
        }
    }

    impl SpatialError for RegErr {}

    impl HyperSpatialError for RegErr {}

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

        pub fn point<S>(point: Point, message: S) -> RegErr
        where
            S: ToString,
        {
            let message = message.to_string();
            RegErr::Point { point, message }
        }

        pub fn pool_not_found<S: ToString>(key: S) -> Self {
            Self::PoolNotFound(key.to_string())
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