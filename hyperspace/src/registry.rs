use crate::base::config::{BaseConfig, BaseSubConfig};
use async_trait::async_trait;
use starlane_space::command::direct::delete::Delete;
use starlane_space::command::direct::query::{Query, QueryResult};
use starlane_space::command::direct::select::{Select, SubSelect};
use starlane_space::hyper::{ParticleLocation, ParticleRecord};
use starlane_space::kind::Kind;
use starlane_space::particle::{Details, Properties, Status, Stub};
use starlane_space::point::Point;
use starlane_space::security::{Access, AccessGrant, IndexedAccessGrant};
use starlane_space::selector::Selector;
use starlane_space::substance::SubstanceList;
use starlane_space::types::property::SetProperties;
use std::sync::Arc;
use starlane_space::err::{HyperSpatialError, ParseErrs0, SpaceErr, SpatialError};
use thiserror::Error;
use serde_derive::{Deserialize, Serialize};
use starlane_space::{SetRegistry, Strategy};

pub type Registry = Arc<dyn RegistryApi>;

pub trait RegistryConfig: BaseSubConfig {}

#[async_trait]
pub trait RegistryApi: Send + Sync {
    async fn scorch<'a>(&'a self) -> Result<(), RegErr>;

    async fn register<'a>(&'a self, registration: &'a Registration) -> Result<(), RegErr>;

    async fn assign_star<'a>(&'a self, point: &'a Point, star: &'a Point) -> Result<(), RegErr>;

    async fn assign_host<'a>(&'a self, point: &'a Point, host: &'a Point) -> Result<(), RegErr>;

    async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), RegErr>;

    async fn set_properties<'a>(
        &'a self,
        point: &'a Point,
        properties: &'a SetProperties,
    ) -> Result<(), RegErr>;

    async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, RegErr>;

    async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, RegErr>;

    async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, RegErr>;

    async fn query<'a>(&'a self, point: &'a Point, query: &'a Query)
        -> Result<QueryResult, RegErr>;

    async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, RegErr>;

    //    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr>;

    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr> {
        let point = select.pattern.query_root();

        let hierarchy = self
            .query(&point, &Query::PointHierarchy)
            .await?
            .try_into()?;

        let sub_select_hops = select.pattern.sub_select_hops();
        let sub_select = select
            .clone()
            .sub_select(point.clone(), sub_select_hops, hierarchy);
        let mut list = self.sub_select(&sub_select).await?;
        if select.pattern.matches_root() {
            list.push(Stub {
                point: Point::root(),
                kind: Kind::Root,
                status: Status::Ready,
            });
        }

        let list = sub_select.into_payload.to_primitive(list)?;

        Ok(list)
    }

    async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, RegErr>;

    async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), RegErr>;

    async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, RegErr>;

    async fn chown<'a>(
        &'a self,
        on: &'a Selector,
        owner: &'a Point,
        by: &'a Point,
    ) -> Result<(), RegErr>;

    async fn list_access<'a>(
        &'a self,
        to: &'a Option<&'a Point>,
        on: &'a Selector,
    ) -> Result<Vec<IndexedAccessGrant>, RegErr>;

    async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), RegErr>;
}

pub struct RegistryWrapper {
    registry: Registry,
}

impl RegistryWrapper {
    pub fn new(registry: Registry) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl RegistryApi for RegistryWrapper {
    async fn scorch<'a>(&'a self) -> Result<(), RegErr> {
        self.registry.scorch().await
    }

    async fn register<'a>(&'a self, registration: &'a Registration) -> Result<(), RegErr> {
        self.registry.register(registration).await
    }

    async fn assign_star<'a>(&'a self, point: &'a Point, star: &'a Point) -> Result<(), RegErr> {
        self.registry.assign_star(point, star).await
    }

    async fn assign_host<'a>(&'a self, point: &'a Point, host: &'a Point) -> Result<(), RegErr> {
        self.registry.assign_host(point, host).await
    }

    async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), RegErr> {
        self.registry.set_status(point, status).await
    }

    async fn set_properties<'a>(
        &'a self,
        point: &'a Point,
        properties: &'a SetProperties,
    ) -> Result<(), RegErr> {
        self.registry.set_properties(point, properties).await
    }

    async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, RegErr> {
        self.registry.sequence(point).await
    }

    async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, RegErr> {
        self.registry.get_properties(point).await
    }

    async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, RegErr> {
        if point.is_global() {
            let location = ParticleLocation::new(Some(Point::local_star()), None);
            let record = ParticleRecord {
                details: Details {
                    stub: Stub {
                        point: point.clone(),
                        kind: Kind::Global,
                        status: Status::Ready,
                    },
                    properties: Properties::default(),
                },
                location,
            };

            Ok(record)
        } else {
            self.registry.record(point).await
        }
    }

    async fn query<'a>(
        &'a self,
        point: &'a Point,
        query: &'a Query,
    ) -> Result<QueryResult, RegErr> {
        self.registry.query(point, query).await
    }

    async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, RegErr> {
        self.registry.delete(delete).await
    }

    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr> {
        self.registry.select(select).await
    }

    async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, RegErr> {
        self.registry.sub_select(sub_select).await
    }

    async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), RegErr> {
        self.registry.grant(access_grant).await
    }

    async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, RegErr> {
        self.registry.access(to, on).await
    }

    async fn chown<'a>(
        &'a self,
        on: &'a Selector,
        owner: &'a Point,
        by: &'a Point,
    ) -> Result<(), RegErr> {
        self.registry.chown(on, owner, by).await
    }

    async fn list_access<'a>(
        &'a self,
        to: &'a Option<&'a Point>,
        on: &'a Selector,
    ) -> Result<Vec<IndexedAccessGrant>, RegErr> {
        self.registry.list_access(to, on).await
    }

    async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), RegErr> {
        self.registry.remove_access(id, to).await
    }
}

#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {}
}

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

pub mod err {}