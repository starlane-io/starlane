use crate::base::config::{BaseConfig, BaseSubConfig};
use async_trait::async_trait;
use serde_derive::{Deserialize, Serialize};
use starlane_space::command::direct::delete::Delete;
use starlane_space::command::direct::query::{Query, QueryResult};
use starlane_space::command::direct::select::{Select, SubSelect};
use starlane_space::err::{HyperSpatialError, ParseErrs0, SpaceErr, SpatialError};
use starlane_space::hyper::{ParticleLocation, ParticleRecord};
use starlane_space::kind::Kind;
use starlane_space::particle::{Details, Properties, Status, Stub};
use starlane_space::point::Point;
use starlane_space::security::{Access, AccessGrant, IndexedAccessGrant};
use starlane_space::selector::Selector;
use starlane_space::substance::SubstanceList;
use starlane_space::types::property::SetProperties;
use starlane_space::{SetRegistry, Strategy};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot::error::RecvError;
use crate::registry::exchange::MuxedRequest;

pub type Registry = Arc<dyn RegistryApi>;

pub trait RegistryConfig: BaseSubConfig {}

#[async_trait]
pub trait RegistryApi: Send + Sync {
    async fn scorch(&self) -> Result<(), RegErr>;

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

    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr>;

    //async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, RegErr>;

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
trait RegistrySubSelect: RegistryApi {
    async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, RegErr>;
}

async fn select<'a>(
    registry: &'a impl RegistrySubSelect,
    select: &'a mut Select,
) -> Result<SubstanceList, RegErr> {
    let point = select.pattern.query_root();

    let hierarchy = registry
        .query(&point, &Query::PointHierarchy)
        .await?
        .try_into()?;

    let sub_select_hops = select.pattern.sub_select_hops();
    let sub_select = select
        .clone()
        .sub_select(point.clone(), sub_select_hops, hierarchy);
    let mut list = registry.sub_select(&sub_select).await?;
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

#[derive(Error, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
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
    #[error("Registry is not reachable")]
    Unreachable,
    #[error("Registry exchanger error")]
    ExchangeErr
}


impl  From<tokio::sync::oneshot::error::RecvError> for RegErr {
    fn from(value: tokio::sync::oneshot::error::RecvError) -> Self {
        Self::ExchangeErr
    }
}




impl From<tokio::sync::mpsc::error::SendError<MuxedRequest>> for RegErr {
    fn from(value: SendError<MuxedRequest>) -> Self {
        RegErr::ExchangeErr
    }
}

impl From<tokio::sync::mpsc::error::SendError<RegistryRequest>> for RegErr {
    fn from(value: SendError<RegistryRequest>) -> Self {
        RegErr::ExchangeErr
    }
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

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Registration {
    pub point: Point,
    pub kind: Kind,
    pub registry: SetRegistry,
    pub properties: SetProperties,
    pub owner: Point,
    pub strategy: Strategy,
    pub status: Status,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, strum_macros::Display)]
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

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, strum_macros::Display)]
pub enum RegistryResponse {
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
            RegistryResponse::ListAccess(value) => Ok(Ok(value)),
            _ => Err(RegErr::msg(
                "Invalid RegistryResponse variant for Result<Vec<IndexedAccessGrant>, RegErr>",
            )),
        }
    }
}

pub mod exchange {
    use crate::registry::{RegErr, Registration, RegistryApi, RegistryRequest, RegistryResponse};
    use dashmap::DashMap;
    use itertools::Itertools;
    use starlane_space::types::registry::Registry;
    use starlane_space::{
        Access, AccessGrant, Delete, IndexedAccessGrant, ParticleRecord, Point, Properties, Query,
        QueryResult, Select, Selector, SetProperties, Status, Stub, SubSelect, SubstanceList,
    };
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::sync::mpsc::error::SendError;

    pub struct MuxExchanger {
        sequence: AtomicU64,
        map: Arc<DashMap<u64, tokio::sync::oneshot::Sender<Result<RegistryResponse, RegErr>>>>,
        tx: tokio::sync::mpsc::Sender<MuxedRequest>,
    }

    impl MuxExchanger {
        pub fn new(exchange_tx: tokio::sync::mpsc::Sender<Exchange>) -> Self {
            let map = Arc::new(DashMap::new());
            let sequence = AtomicU64::new(0u64);
            let (req_tx, req_rx) = tokio::sync::mpsc::channel(128);
            MuxRunner::new(exchange_tx, map.clone(), req_rx);
            Self {
                sequence,
                map,
                tx: req_tx
            }
        }
        pub async fn request(&self, request: RegistryRequest) -> Result<RegistryResponse, RegErr> {
            let id = self.sequence.fetch_add(1u64, Ordering::Relaxed);
            let request = MuxedRequest::new(request, id);
            let (res_tx, res_rx) = tokio::sync::oneshot::channel();
            self.map.insert(id, res_tx);
            self.tx
                .send(request)
                .await
                .map_err(|_| RegErr::Unreachable)?;
            res_rx
                .await
                .map_err(|_| RegErr::Unreachable)
                .map(|res| res.unwrap())
        }
    }

    struct MuxRunner {
        mux_rx: tokio::sync::mpsc::Receiver<MuxedRequest>,
        exchange_tx: tokio::sync::mpsc::Sender<Exchange>,
        map: Arc<DashMap<u64, tokio::sync::oneshot::Sender<Result<RegistryResponse, RegErr>>>>,
    }

    impl MuxRunner {
        pub fn new(
            exchange_tx: tokio::sync::mpsc::Sender<Exchange>,
            map: Arc<DashMap<u64, tokio::sync::oneshot::Sender<Result<RegistryResponse, RegErr>>>>,
            mux_rx: tokio::sync::mpsc::Receiver<MuxedRequest>,
        ) {
            let mut runner = Self {
                mux_rx,
                exchange_tx,
                map,
            };

            tokio::spawn(async move { runner.start().await });
        }

        async fn start(mut self) {
            while let Some(mux_req) = self.mux_rx.recv().await {
                let (exchange, mut rx) = Exchange::new(mux_req.request);
                let map = self.map.clone();
                self.exchange_tx.send(exchange).await;
                tokio::spawn(async move {
                    let result = rx
                        .await
                        .map_err(|_| RegErr::Unreachable)
                        .map(|r| r.unwrap());
                    if let Some((_, tx)) = map.remove(&mux_req.id) {
                        tx.send(result).unwrap_or_default()
                    }
                });
            }
        }
    }

    pub struct MuxedRequest {
        id: u64,
        request: RegistryRequest,
    }

    impl MuxedRequest {
        pub fn new(request: RegistryRequest, id: u64) -> Self {
            Self { id, request }
        }
    }

    pub struct MuxedResult {
        id: u64,
        result: Result<RegistryResponse, RegErr>,
    }

    pub struct RegistryExchanger {
        tx: tokio::sync::mpsc::Sender<Exchange>,
    }

    impl RegistryExchanger {
        pub fn new(registry: impl Into<Arc<dyn RegistryApi>>) -> Self {
            let tx = RegistryRunner::new(registry);
            Self { tx }
        }
    }

    #[async_trait::async_trait]
    impl RegistryApi for RegistryExchanger {
        async fn scorch<'a>(&'a self) -> Result<(), RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::Scorch);
            self.tx.send(x).await?;
            if let RegistryResponse::Scorch = rx.await?? {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn register<'a>(&'a self, registration: &'a Registration) -> Result<(), RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::Register(registration.clone()));
            self.tx.send(x).await?;
            if let RegistryResponse::Register= rx.await?? {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn assign_star<'a>(
            &'a self,
            point: &'a Point,
            star: &'a Point,
        ) -> Result<(), RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::AssignStar {
                point: point.clone(),
                star: star.clone(),
            });
            self.tx.send(x).await?;
            if let RegistryResponse::AssignStar= rx.await?? {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn assign_host<'a>(
            &'a self,
            point: &'a Point,
            host: &'a Point,
        ) -> Result<(), RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::AssignHost {
                point: point.clone(),
                host: host.clone(),
            });
            self.tx.send(x).await?;
            if let RegistryResponse::AssignHost= rx.await?? {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn set_status<'a>(
            &'a self,
            point: &'a Point,
            status: &'a Status,
        ) -> Result<(), RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::SetStatus {
                point: point.clone(),
                status: status.clone(),
            });
            self.tx.send(x).await?;
            if let RegistryResponse::SetStatus = rx.await?? {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn set_properties<'a>(
            &'a self,
            point: &'a Point,
            properties: &'a SetProperties,
        ) -> Result<(), RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::SetProperties {
                point: point.clone(),
                properties: properties.clone(),
            });
            self.tx.send(x).await?;
            if let RegistryResponse::SetProperties= rx.await?? {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::Sequence(point.clone()));
            self.tx.send(x).await?;
            if let RegistryResponse::Sequence(ret)= rx.await?? {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::GetProperties(point.clone()));
            self.tx.send(x).await?;
            if let RegistryResponse::GetProperties(ret)= rx.await?? {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::Record(point.clone()));
            self.tx.send(x).await.map_err(|_| RegErr::Unreachable)?;

            if let RegistryResponse::Record(ret)= rx.await?? {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn query<'a>(
            &'a self,
            point: &'a Point,
            query: &'a Query,
        ) -> Result<QueryResult, RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::Query {
                point: point.clone(),
                query: query.clone(),
            });
            self.tx.send(x).await?;
            if let RegistryResponse::Query(ret)= rx.await?? {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::Delete(delete.clone()));
            self.tx.send(x).await?;
            if let RegistryResponse::Delete(ret)= rx.await?? {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::Select(select.clone()));
            self.tx.send(x).await?;
            if let RegistryResponse::Select(ret)= rx.await?? {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::Grant(access_grant.clone()));
            self.tx.send(x).await?;
            if let RegistryResponse::Grant= rx.await?? {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::Access {
                to: to.clone(),
                on: on.clone(),
            });
            self.tx.send(x).await?;
            if let RegistryResponse::Access(ret)= rx.await?? {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn chown<'a>(
            &'a self,
            on: &'a Selector,
            owner: &'a Point,
            by: &'a Point,
        ) -> Result<(), RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::Chown {
                on: on.clone(),
                owner: owner.clone(),
                by: by.clone(),
            });
            self.tx.send(x).await?;
            if let RegistryResponse::Chown= rx.await?? {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn list_access<'a>(
            &'a self,
            to: &'a Option<&'a Point>,
            on: &'a Selector,
        ) -> Result<Vec<IndexedAccessGrant>, RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::ListAccess {
                to: to.map(|p| p.clone()),
                on: on.clone(),
            });
            self.tx.send(x).await?;
            if let RegistryResponse::ListAccess(ret)= rx.await?? {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), RegErr> {
            let (x, rx) = Exchange::new(RegistryRequest::RemoveAccess { id, to: to.clone() });
            self.tx.send(x).await?;
            if let RegistryResponse::RemoveAccess= rx.await?? {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }
    }

    struct Exchange {
        pub request: RegistryRequest,
        pub tx: tokio::sync::oneshot::Sender<Result<RegistryResponse, RegErr>>,
    }

    impl From<tokio::sync::mpsc::error::SendError<Exchange>> for RegErr {
        fn from(value: SendError<Exchange>) -> Self {
            RegErr::ExchangeErr
        }
    }

    impl Exchange {
        pub fn new(
            request: RegistryRequest,
        ) -> (
            Self,
            tokio::sync::oneshot::Receiver<Result<RegistryResponse, RegErr>>,
        ) {
            let (tx, rx) = tokio::sync::oneshot::channel();
            (Self { request, tx }, rx)
        }
    }

    struct RegistryRunner {
        rx: tokio::sync::mpsc::Receiver<Exchange>,
        registry: Arc<dyn RegistryApi>,
    }

    impl RegistryRunner {
        pub fn new(registry: impl Into<Arc<dyn RegistryApi>>) -> tokio::sync::mpsc::Sender<Exchange> {
            let (tx, rx) = tokio::sync::mpsc::channel(128);
            let registry = registry.into();
            let runner = Self { rx, registry };
            tx
        }
        pub async fn start(mut self) {
            while let Some(x) = self.rx.recv().await {
                let registry = self.registry.clone();
                tokio::spawn(async move {
                    let result: Result<RegistryResponse, RegErr> = match x.request {
                        RegistryRequest::Scorch => {
                            registry.scorch().await.map(|_| RegistryResponse::Scorch)
                        }
                        RegistryRequest::Register(registration) => registry
                            .register(&registration)
                            .await
                            .map(|_| RegistryResponse::Register),
                        RegistryRequest::AssignStar { point, star } => registry
                            .assign_star(&point, &star)
                            .await
                            .map(|_| RegistryResponse::AssignStar),
                        RegistryRequest::AssignHost { point, host } => registry
                            .assign_host(&point, &host)
                            .await
                            .map(|_| RegistryResponse::AssignHost),
                        RegistryRequest::SetStatus { point, status } => registry
                            .set_status(&point, &status)
                            .await
                            .map(|_| RegistryResponse::SetStatus),
                        RegistryRequest::SetProperties { point, properties } => registry
                            .set_properties(&point, &properties)
                            .await
                            .map(|_| RegistryResponse::SetProperties),
                        RegistryRequest::Sequence(point) => registry
                            .sequence(&point)
                            .await
                            .map(|value| RegistryResponse::Sequence(value)),
                        RegistryRequest::GetProperties(point) => registry
                            .get_properties(&point)
                            .await
                            .map(|value| RegistryResponse::GetProperties(value)),
                        RegistryRequest::Record(point) => registry
                            .record(&point)
                            .await
                            .map(|value| RegistryResponse::Record(value)),
                        RegistryRequest::Query { point, query } => registry
                            .query(&point, &query)
                            .await
                            .map(|value| RegistryResponse::Query(value)),
                        RegistryRequest::Delete(delete) => registry
                            .delete(&delete)
                            .await
                            .map(|value| RegistryResponse::Delete(value)),
                        RegistryRequest::Select(mut select) => registry
                            .select(&mut select)
                            .await
                            .map(|value| RegistryResponse::Select(value)),
                        RegistryRequest::Grant(access_grant) => registry
                            .grant(&access_grant)
                            .await
                            .map(|_| RegistryResponse::Grant),
                        RegistryRequest::Access { to, on } => registry
                            .access(&to, &on)
                            .await
                            .map(|value| RegistryResponse::Access(value)),
                        RegistryRequest::Chown { on, owner, by } => registry
                            .chown(&on, &owner, &by)
                            .await
                            .map(|_| RegistryResponse::Chown),
                        RegistryRequest::ListAccess { to, on } => registry
                            .list_access(&to.as_ref(), &on)
                            .await
                            .map(|value| RegistryResponse::ListAccess(value)),
                        RegistryRequest::RemoveAccess { id, to } => registry
                            .remove_access(id, &to)
                            .await
                            .map(|_| RegistryResponse::RemoveAccess),
                    };
                    x.tx.send(result).unwrap();
                });
            }
        }
    }
}
