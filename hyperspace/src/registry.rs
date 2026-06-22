use crate::base::config::{BaseConfig, BaseSubConfig};
use crate::registry::exchange::{Exchange, MuxedRequest, MuxedResult, RegistryResult};
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
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot::error::RecvError;

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
    ExchangeErr,
}

impl From<tokio::sync::oneshot::error::RecvError> for RegErr {
    fn from(value: tokio::sync::oneshot::error::RecvError) -> Self {
        Self::ExchangeErr
    }
}

impl From<tokio::sync::mpsc::error::SendError<RegistryRequest>> for RegErr {
    fn from(value: SendError<RegistryRequest>) -> Self {
        Self::ExchangeErr
    }
}
impl From<tokio::sync::mpsc::error::SendError<RegistryResponse>> for RegErr {
    fn from(value: SendError<RegistryResponse>) -> Self {
        Self::ExchangeErr
    }
}

impl From<tokio::sync::mpsc::error::SendError<RegistryResult>> for RegErr {
    fn from(value: SendError<RegistryResult>) -> Self {
        Self::ExchangeErr
    }
}

impl From<tokio::sync::mpsc::error::SendError<MuxedRequest>> for RegErr {
    fn from(value: SendError<MuxedRequest>) -> Self {
        Self::ExchangeErr
    }
}

impl From<tokio::sync::mpsc::error::SendError<MuxedResult>> for RegErr {
    fn from(value: SendError<MuxedResult>) -> Self {
        Self::ExchangeErr
    }
}

impl From<tokio::sync::mpsc::error::SendError<Exchange>> for RegErr {
    fn from(value: SendError<Exchange>) -> Self {
        Self::ExchangeErr
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

impl Registration {
    pub fn mock() -> Self {
        Self {
            point: Point::root(),
            kind: Kind::Root,
            registry: Default::default(),
            properties: Default::default(),
            owner: Point::hyper_user(),
            strategy: Strategy::Commit,
            status: Status::Unknown,
        }
    }
}

#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    strum_macros::Display,
    strum_macros::EnumDiscriminants,
)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(RegistryRequestType))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize, strum_macros::Display))]
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

#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    strum_macros::Display,
    strum_macros::EnumDiscriminants,
)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(RegistryResponseType))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize, strum_macros::Display))]
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
    use anyhow::{anyhow, Error};
    use async_trait::async_trait;
    use dashmap::DashMap;
    use futures::{Sink, SinkExt, Stream, StreamExt};
    use itertools::Itertools;

    use nom::AsBytes;
    use serde::{Deserialize, Serialize};
    use starlane_space::status::{status_reporter, StatusDetail, StatusReport, StatusReporter};
    use starlane_space::types::registry::Registry;
    use starlane_space::{
        Access, AccessGrant, Delete, IndexedAccessGrant, ParticleRecord, Point, Properties, Query,
        QueryResult, Select, Selector, SetProperties, Status, Stub, SubSelect, SubstanceList,
    };
    use std::collections::HashMap;
    use std::fmt::{Display, Formatter};
    use std::io;
    use std::io::Write;
    use std::marker::PhantomData;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::task::{Context, Poll};
    use std::time::Duration;
    use thiserror::Error;
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
    use tokio::sync::mpsc::error::SendError;
    use tokio_pipe::PipeWrite;
    use tokio_util::bytes::{BufMut, BytesMut};
    use tokio_util::codec::{
        Decoder, Encoder, Framed, FramedRead, FramedWrite, LengthDelimitedCodec, LinesCodec,
    };

    mod transform {
        use super::*;

        pub fn scorch(result: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::Scorch = result {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn register(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::Register = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn assign_star(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::AssignStar = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }
        pub fn assign_host(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::AssignHost = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn set_status(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::SetStatus = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn set_properties(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::SetProperties = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn sequence(res: RegistryResponse) -> Result<u64, RegErr> {
            if let RegistryResponse::Sequence(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn get_properties(res: RegistryResponse) -> Result<Properties, RegErr> {
            if let RegistryResponse::GetProperties(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn record(res: RegistryResponse) -> Result<ParticleRecord, RegErr> {
            if let RegistryResponse::Record(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn query(res: RegistryResponse) -> Result<QueryResult, RegErr> {
            if let RegistryResponse::Query(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn delete(res: RegistryResponse) -> Result<SubstanceList, RegErr> {
            if let RegistryResponse::Delete(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn select(res: RegistryResponse) -> Result<SubstanceList, RegErr> {
            if let RegistryResponse::Select(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn grant(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::Grant = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn access(res: RegistryResponse) -> Result<Access, RegErr> {
            if let RegistryResponse::Access(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn chown(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::Chown = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn list_access(res: RegistryResponse) -> Result<Vec<IndexedAccessGrant>, RegErr> {
            if let RegistryResponse::ListAccess(ret) = res {
                Ok(ret)
            } else {
                Err(RegErr::ExchangeErr)
            }
        }

        pub fn remove_access(res: RegistryResponse) -> Result<(), RegErr> {
            if let RegistryResponse::RemoveAccess = res {
                Ok(())
            } else {
                Err(RegErr::ExchangeErr)
            }
        }
    }

    pub struct MuxFramedWriter<T, S>
    where
        T: Send + Sync,
        S: SinkExt<T> + Sink<T, Error = anyhow::Error> + Send + Sync + std::marker::Unpin,
    {
        rx: tokio::sync::mpsc::Receiver<T>,
        sink: S,
    }

    impl<T, S> MuxFramedWriter<T, S>
    where
        T: Send + Sync + 'static,
        S: Sink<T, Error = anyhow::Error> + Send + Sync + std::marker::Unpin + 'static,
    {
        pub fn new(sink: S) -> tokio::sync::mpsc::Sender<T>
        where
            T: Send + Sync,
            S: Sink<T, Error = anyhow::Error> + Send + Sync + std::marker::Unpin,
        {
            let (tx, rx) = tokio::sync::mpsc::channel(128);
            let writer = Self { rx, sink };
            tokio::spawn(async move { writer.start().await });
            tx
        }

        async fn start(mut self) {
            while let Some(frame) = self.rx.recv().await {
                if let Err(err) = self.sink.send(frame).await {
                    println!("error sending frame: {}", err);
                    break;
                }
            }
            println!("terminating MuxFramedWriter...");
        }
    }

    pub struct MuxFramedReader<T, S>
    where
        T: Display + Send + Sync + 'static,
        S: StreamExt<Item = Result<T, anyhow::Error>> + Send + Sync + std::marker::Unpin + 'static,
    {
        tx: tokio::sync::mpsc::Sender<T>,
        stream: S,
    }

    impl<T, S> MuxFramedReader<T, S>
    where
        T: Send + Sync + Display,
        S: StreamExt<Item = Result<T, anyhow::Error>> + Send + Sync + std::marker::Unpin + 'static,
    {
        pub fn new(stream: S) -> tokio::sync::mpsc::Receiver<T> {
            let (tx, rx) = tokio::sync::mpsc::channel(128);
            let reader = Self { tx, stream };
            tokio::spawn(async move { reader.start().await });
            rx
        }

        async fn start(mut self) {
            loop {
                match self.stream.next().await {
                    Some(Ok(t)) => {
                        if let Err(err) = self.tx.send(t).await {
                            println!("read send err...{}", err);
                            break;
                        }
                    }
                    Some(Err(err)) => {
                        println!("read err: {}", err);
                        break;
                    }
                    None => {
                        println!("none");
                        break;
                    }
                }
            }
            println!("terminating MuxFramedReader...");
        }
    }

    pub struct MuxRegistryClient {
        sequence: AtomicU64,
        map: Arc<DashMap<u64, tokio::sync::oneshot::Sender<Signal<RegistryResult>>>>,
        sink: tokio::sync::mpsc::Sender<MuxedRequest>,
    }

    impl MuxRegistryClient {
        pub fn local(registry: Arc<dyn RegistryApi>) -> Self {
            let (client_request_sink, client_request_stream) = tokio::sync::mpsc::channel(128);

            let (server_response_sink, server_response_stream) = tokio::sync::mpsc::channel(128);

            MuxRegistryServer::new(registry, client_request_stream, server_response_sink);

            Self::new(client_request_sink, server_response_stream)
        }

        pub fn from_connection<R, W>(read: R, write: W) -> Self
        where
            R: AsyncRead + Send + Sync + Unpin + 'static,
            W: AsyncWrite + Send + Sync + Unpin + 'static,
        {
            let sink = MuxFramedWriter::new(FramedWrite::new(write, MuxedRequestCodec::default()));
            let stream = MuxFramedReader::new(FramedRead::new(read, MuxedResultCodec::default()));
            Self::new(sink, stream)
        }

        pub fn new(
            sink: tokio::sync::mpsc::Sender<MuxedRequest>,
            stream: tokio::sync::mpsc::Receiver<MuxedResult>,
        ) -> Self {
            /// start the receiver
            let map = Arc::new(DashMap::new());
            MuxResultReceiver::new(stream, map.clone());

            let sequence = AtomicU64::new(0u64);
            Self {
                sequence,
                map,
                sink,
            }
        }
    }

    #[async_trait]
    impl Sender for MuxRegistryClient {
        async fn signal<R, F>(
            &self,
            signal: Signal<RegistryRequest>,
            expect: F,
        ) -> Result<R, RegErr>
        where
            F: Fn(Signal<RegistryResult>) -> Result<R, RegErr> + Send + Sync,
        {
            let id = self.sequence.fetch_add(1u64, Ordering::Relaxed);
            let request = MuxedRequest::signal(id, signal);
            let (res_tx, res_rx) = tokio::sync::oneshot::channel();
            self.map.insert(id, res_tx);
            self.sink.send(request).await?;
            let response = res_rx.await?;
            expect(response)
        }
    }

    struct MuxResultReceiver {
        stream: tokio::sync::mpsc::Receiver<MuxedResult>,
        map: Arc<DashMap<u64, tokio::sync::oneshot::Sender<Signal<RegistryResult>>>>,
    }

    impl MuxResultReceiver {
        pub fn new(
            stream: tokio::sync::mpsc::Receiver<MuxedResult>,
            map: Arc<DashMap<u64, tokio::sync::oneshot::Sender<Signal<RegistryResult>>>>,
        ) {
            let mut runner = Self { stream, map };

            tokio::spawn(async move { runner.start().await });
        }

        async fn start(mut self) {
            while let Some(res) = self.stream.recv().await {
                if let Some((_, tx)) = self.map.remove(&res.id) {
                    tx.send(res.signal);
                }
            }
        }
    }

    pub struct MuxRegistryServer {
        sink_tx: tokio::sync::mpsc::Sender<MuxedResult>,
        stream: tokio::sync::mpsc::Receiver<MuxedRequest>,
        tx: tokio::sync::mpsc::Sender<Exchange>,
    }

    impl MuxRegistryServer {
        pub fn from_connection<R, W>(registry: Arc<dyn RegistryApi>, read: R, write: W)
        where
            R: AsyncRead + Send + Sync + Unpin + 'static,
            W: AsyncWrite + Send + Sync + Unpin + 'static,
        {
            let sink = MuxFramedWriter::new(FramedWrite::new(write, MuxedResultCodec::default()));
            let stream = MuxFramedReader::new(FramedRead::new(read, MuxedRequestCodec::default()));
            Self::new(registry, stream, sink);
        }

        pub fn new(
            registry: Arc<dyn RegistryApi>,
            stream: tokio::sync::mpsc::Receiver<MuxedRequest>,
            sink: tokio::sync::mpsc::Sender<MuxedResult>,
        ) {
            let tx = ExchangeRunner::new(registry);
            let (sink_tx, mut sink_rx) = tokio::sync::mpsc::channel(100);
            tokio::spawn(async move {
                while let Some(frame) = sink_rx.recv().await {
                    if let Err(_) = sink.send(frame).await {
                        break;
                    }
                }
            });
            let mut server = Self {
                stream,
                sink_tx,
                tx,
            };
            tokio::spawn(async move { server.start().await });
        }

        async fn start(mut self) {
            while let Some(MuxedRequest { id, signal }) = self.stream.recv().await {
                let sink = self.sink_tx.clone();
                let (exchange, rx) = Exchange::signal(signal);
                self.tx.send(exchange).await.unwrap();
                tokio::spawn(async move {
                    let result = rx.await.unwrap();
                    let mux_out = MuxedResult::signal(id, result);
                    sink.send(mux_out).await;
                });
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MuxedSignal<T> {
        pub id: u64,
        pub signal: Signal<T>,
    }

    impl<T> Display for MuxedSignal<T>
    where
        T: Display,
    {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "MuxedSignal({})<{}>", self.id, self.signal)
        }
    }

    impl<T> MuxedSignal<T> {
        pub fn transport(id: u64, t: T) -> Self {
            Self::signal(id, Signal::Transport(t))
        }

        pub fn signal(id: u64, signal: Signal<T>) -> Self {
            Self { id, signal }
        }
    }

    pub type MuxedRequest = MuxedSignal<RegistryRequest>;
    pub type MuxedResult = MuxedSignal<RegistryResult>;

    #[derive(
        Debug, Serialize, Deserialize, Clone, strum_macros::EnumDiscriminants, strum_macros::Display,
    )]
    #[strum_discriminants(vis(pub))]
    #[strum_discriminants(name(RegistryResultType))]
    #[strum_discriminants(derive(Hash, Serialize, Deserialize, strum_macros::Display))]
    pub enum RegistryResult {
        Ok(RegistryResponse),
        Err(RegErr),
    }

    impl RegistryResult {
        pub fn into_result(self) -> Result<RegistryResponse, RegErr> {
            match self {
                RegistryResult::Ok(value) => Ok(value),
                RegistryResult::Err(value) => Err(value),
            }
        }
    }

    impl From<Result<RegistryResponse, RegErr>> for RegistryResult {
        fn from(result: Result<RegistryResponse, RegErr>) -> Self {
            match result {
                Ok(result) => Self::Ok(result),
                Err(err) => Self::Err(err),
            }
        }
    }

    impl Into<Result<RegistryResponse, RegErr>> for RegistryResult {
        fn into(self) -> Result<RegistryResponse, RegErr> {
            match self {
                RegistryResult::Ok(value) => Ok(value),
                RegistryResult::Err(value) => Err(value),
            }
        }
    }

    /*
    #[derive(Default)]
    struct MuxedRequestCodec(LengthDelimitedCodec);

    impl Decoder for MuxedRequestCodec {
        type Item = MuxedRequest;
        type Error = anyhow::Error;

        fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            use tokio_util::bytes::Buf;
            let mut data = self.0.decode(src)?.ok_or(anyhow::Error::msg("No data"))?;
            let read = bincode::deserialize(& mut data)?;
            Ok(Some(read))
        }
    }

    impl Encoder<MuxedRequest> for MuxedRequestCodec {
        type Error = anyhow::Error;

        fn encode(&mut self, item: MuxedRequest, dst: &mut BytesMut) -> Result<(), Self::Error> {
            let data = bincode::serialize(& item)?;
            self.0.encode(data.into(),dst)?;
            Ok(())
        }
    }

     */
    pub type MuxedRequestCodec = SerdeCodec<MuxedRequest>;
    pub type MuxedResultCodec = SerdeCodec<MuxedResult>;

    pub struct SerdeCodec<T>
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        length_codec: LengthDelimitedCodec,
        _phantom: PhantomData<T>,
    }

    impl<T> Default for SerdeCodec<T>
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        fn default() -> Self {
            Self {
                length_codec: LengthDelimitedCodec::default(),
                _phantom: PhantomData::default(),
            }
        }
    }

    impl<T> Encoder<T> for SerdeCodec<T>
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        type Error = anyhow::Error;

        fn encode(&mut self, item: T, dst: &mut BytesMut) -> Result<(), Self::Error> {
            let data = bincode::serialize(&item)?;
            self.length_codec.encode(data.into(), dst)?;
            Ok(())
        }
    }
    impl<T> Decoder for SerdeCodec<T>
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        type Item = T;
        type Error = anyhow::Error;

        fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            if let Some(mut data) = self.length_codec.decode(src)? {
                let t = bincode::deserialize(&mut data)?;
                Ok(Some(t))
            } else {
                Ok(None)
            }
        }
    }

    #[async_trait]
    trait Sender: Send + Sync {
        async fn send<R, F>(&self, request: RegistryRequest, expect: F) -> Result<R, RegErr>
        where
            F: Fn(RegistryResponse) -> Result<R, RegErr> + Send + Sync,
        {
            self.signal(Signal::Transport(request), move |signal| {
                let request = signal
                    .transport_or()
                    .map_err(|_| RegErr::ExchangeErr)
                    .map(RegistryResult::into_result)??;
                expect(request)
            })
            .await
        }

        async fn signal<R, F>(
            &self,
            signal: Signal<RegistryRequest>,
            expect: F,
        ) -> Result<R, RegErr>
        where
            F: Fn(Signal<RegistryResult>) -> Result<R, RegErr> + Send + Sync;
    }

    #[async_trait]
    impl<S: Sender> RegistryApi for S {
        async fn scorch<'a>(&'a self) -> Result<(), RegErr> {
            self.send(RegistryRequest::Scorch, transform::scorch).await
        }

        async fn register<'a>(&'a self, registration: &'a Registration) -> Result<(), RegErr> {
            let registration = registration.clone();
            self.send(RegistryRequest::Register(registration), transform::register)
                .await
        }

        async fn assign_star<'a>(
            &'a self,
            point: &'a Point,
            star: &'a Point,
        ) -> Result<(), RegErr> {
            let point = point.clone();
            let star = star.clone();
            self.send(
                RegistryRequest::AssignStar { point, star },
                transform::assign_star,
            )
            .await
        }

        async fn assign_host<'a>(
            &'a self,
            point: &'a Point,
            host: &'a Point,
        ) -> Result<(), RegErr> {
            let point = point.clone();
            let host = host.clone();
            self.send(
                RegistryRequest::AssignHost { point, host },
                transform::assign_host,
            )
            .await
        }

        async fn set_status<'a>(
            &'a self,
            point: &'a Point,
            status: &'a Status,
        ) -> Result<(), RegErr> {
            let point = point.clone();
            let status = status.clone();
            self.send(
                RegistryRequest::SetStatus { point, status },
                transform::set_status,
            )
            .await
        }

        async fn set_properties<'a>(
            &'a self,
            point: &'a Point,
            properties: &'a SetProperties,
        ) -> Result<(), RegErr> {
            let point = point.clone();
            let properties = properties.clone();
            let request = RegistryRequest::SetProperties { point, properties };
            self.send(request, transform::set_properties).await
        }

        async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, RegErr> {
            let point = point.clone();
            let request = RegistryRequest::Sequence(point);
            self.send(request, transform::sequence).await
        }

        async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, RegErr> {
            let point = point.clone();
            let request = RegistryRequest::GetProperties(point);
            self.send(request, transform::get_properties).await
        }

        async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, RegErr> {
            let point = point.clone();
            let request = RegistryRequest::Record(point);
            self.send(request, transform::record).await
        }

        async fn query<'a>(
            &'a self,
            point: &'a Point,
            query: &'a Query,
        ) -> Result<QueryResult, RegErr> {
            let point = point.clone();
            let query = query.clone();
            let request = RegistryRequest::Query { point, query };
            self.send(request, transform::query).await
        }

        async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, RegErr> {
            let delete = delete.clone();
            let request = RegistryRequest::Delete(delete);
            self.send(request, transform::delete).await
        }

        async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr> {
            let select = select.clone();
            let request = RegistryRequest::Select(select);
            self.send(request, transform::select).await
        }

        async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), RegErr> {
            let access_grant = access_grant.clone();
            let request = RegistryRequest::Grant(access_grant);
            self.send(request, transform::grant).await
        }

        async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, RegErr> {
            let to = to.clone();
            let on = on.clone();
            let request = RegistryRequest::Access { to, on };
            self.send(request, transform::access).await
        }

        async fn chown<'a>(
            &'a self,
            on: &'a Selector,
            owner: &'a Point,
            by: &'a Point,
        ) -> Result<(), RegErr> {
            let on = on.clone();
            let owner = owner.clone();
            let by = by.clone();
            let request = RegistryRequest::Chown { on, owner, by };
            self.send(request, transform::chown).await
        }

        async fn list_access<'a>(
            &'a self,
            to: &'a Option<&'a Point>,
            on: &'a Selector,
        ) -> Result<Vec<IndexedAccessGrant>, RegErr> {
            let to = to.map(|p| p.clone());
            let on = on.clone();
            let request = RegistryRequest::ListAccess { to, on };
            self.send(request, transform::list_access).await
        }

        async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), RegErr> {
            let to = to.clone();
            let request = RegistryRequest::RemoveAccess { id, to };
            self.send(request, transform::remove_access).await
        }
    }

    pub struct RegistryExchanger {
        tx: tokio::sync::mpsc::Sender<Exchange>,
        status: StatusReporter,
    }

    impl RegistryExchanger {
        pub fn new(registry: Arc<dyn RegistryApi>) -> Self {
            let tx = ExchangeRunner::new(registry);
            let status = status_reporter();
            Self { tx, status }
        }
    }

    #[async_trait]
    impl Sender for RegistryExchanger {
        /*
        async fn send<R, F>(&self, request: RegistryRequest, expect: F) -> Result<R, RegErr>
        where
            F: Fn(RegistryResponse) -> Result<R, RegErr> + Send + Sync,
        {
            let (exchange, mut rx) = Exchange::request(request);
            self.tx.send(exchange).await?;
            let result = rx.await?;
            expect(result)
        }

         */

        async fn signal<R, F>(
            &self,
            signal: Signal<RegistryRequest>,
            expect: F,
        ) -> Result<R, RegErr>
        where
            F: Fn(Signal<RegistryResult>) -> Result<R, RegErr> + Send + Sync,
        {
            let (exchange, mut rx) = Exchange::signal(signal);
            self.tx.send(exchange).await?;
            let result = rx.await?;
            expect(result)
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub enum Signal<T> {
        Probe(Probe),
        Transport(T),
    }

    impl<T> Display for Signal<T>
    where
        T: Display,
    {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                Signal::Probe(t) => {
                    write!(f, "Signal::Trace({})", t)
                }
                Signal::Transport(t) => {
                    write!(f, "Signal::Transport({})", t)
                }
            }
        }
    }

    impl<T> Signal<T>
    where
        T: Display,
    {
        pub fn transport_or(self) -> Result<T, anyhow::Error> {
            match self {
                Signal::Transport(t) => Ok(t),
                _ => Err(anyhow!("expected a transport")),
            }
        }
        pub fn unwrap(self) -> T {
            match self {
                Signal::Transport(t) => t,
                _ => panic!("expected transport"),
            }
        }
    }

    impl From<Result<RegistryResponse, RegErr>> for Signal<Result<RegistryResponse, RegErr>> {
        fn from(value: Result<RegistryResponse, RegErr>) -> Self {
            Signal::Transport(value)
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub enum Probe {
        /// return a stack of names
        Status(Vec<String>),
        Report(Vec<StatusReport>),
    }

    impl Display for Probe {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                Probe::Status(_) => write!(f, "Probe::Probe"),
                Probe::Report(_) => write!(f, "Probe::Return"),
            }
        }
    }

    impl Probe {
        pub fn push(mut self, name: &str) -> anyhow::Result<Self> {
            if let Self::Status(mut stack) = self {
                stack.push(name.to_string());
                Ok(Self::Status(stack))
            } else {
                Err(anyhow!("not a probe"))
            }
        }
    }

    pub struct Exchange {
        pub signal: Signal<RegistryRequest>,
        pub tx: tokio::sync::oneshot::Sender<Signal<RegistryResult>>,
    }

    impl Exchange {
        pub fn request(
            request: RegistryRequest,
        ) -> (Self, tokio::sync::oneshot::Receiver<Signal<RegistryResult>>) {
            let signal = Signal::Transport(request);
            Self::signal(signal)
        }

        pub fn signal(
            signal: Signal<RegistryRequest>,
        ) -> (Self, tokio::sync::oneshot::Receiver<Signal<RegistryResult>>) {
            let (tx, rx) = tokio::sync::oneshot::channel();
            (Self { signal, tx }, rx)
        }
    }

    struct ExchangeRunner {
        rx: tokio::sync::mpsc::Receiver<Exchange>,
        registry: Arc<dyn RegistryApi>,
    }

    impl ExchangeRunner {
        pub fn new(registry: Arc<dyn RegistryApi>) -> tokio::sync::mpsc::Sender<Exchange> {
            let (tx, rx) = tokio::sync::mpsc::channel(128);
            let mut runner = Self { rx, registry };
            tokio::spawn(async move {
                runner.start().await;
            });
            tx
        }
        pub async fn start(mut self) {
            while let Some(x) = self.rx.recv().await {
                let registry = self.registry.clone();
                tokio::spawn(async move {
                    match x.signal {
                        Signal::Transport(request) => {
                            let result: Result<RegistryResponse, RegErr> = match request {
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
                            let result = Signal::Transport(RegistryResult::from(result));
                            x.tx.send(result).unwrap();
                        }
                        Signal::Probe(Trace) => {
                            panic!("cannot handle Signal::Probe yet")
                        }
                    }
                });
            }
        }
    }

    #[derive(Error, Debug)]
    enum TxRxErr {}
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::hyperlane::HyperwayKind::Mount;
    use crate::registry::exchange::{
        MuxRegistryClient, MuxRegistryServer, MuxedRequest, RegistryExchanger,
    };
    use mockall::mock;
    use starlane_space::wave::exchange::asynch::Exchanger;
    use std::time::Duration;
    use tokio_util::codec::{FramedRead, FramedWrite};

    mock! {
        pub Registry{
        }

        #[async_trait]
        impl RegistryApi for Registry{
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

        async fn delete<'a>(&'a self, d: &'a Delete) -> Result<SubstanceList, RegErr>;

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
    }
    #[tokio::test]
    pub async fn test_mock() {
        let mock = mock();
        //        let registry = RegistryExchanger::new(mock);
        test_registry(mock).await.unwrap();
    }

    #[tokio::test]
    pub async fn test_exchanger() {
        let mock = Arc::new(mock());
        let registry = RegistryExchanger::new(mock.clone());
        test_registry(registry).await.unwrap();
        drop(mock);
    }

    #[tokio::test]
    pub async fn test_muxer() {
        let blah = Arc::new(1u64);

        let mock = Arc::new(mock());
        let registry = MuxRegistryClient::local(mock.clone());
        test_registry(registry).await.unwrap();
    }

    #[tokio::test]
    pub async fn test_muxer_over_pipes() {
        let mock = Arc::new(mock());
        let registry = MuxRegistryClient::local(mock.clone());
        let (client_request_read, client_request_write) = tokio_pipe::pipe().unwrap();
        let (server_result_read, server_result_write) = tokio_pipe::pipe().unwrap();
        MuxRegistryServer::from_connection(mock, client_request_read, server_result_write);
        let registry = MuxRegistryClient::from_connection(server_result_read, client_request_write);

        tokio::time::timeout(Duration::from_secs(15), test_registry(registry))
            .await
            .unwrap()
            .unwrap();
    }

    pub fn mock() -> MockRegistry {
        let mut mock = MockRegistry::new();
        mock.expect_scorch().times(1).returning(|| Ok(()));
        mock.expect_register().times(1).returning(|x| Ok(()));

        mock.expect_assign_host().times(1).returning(|_, _| Ok(()));
        mock.expect_assign_star().times(1).returning(|_, _| Ok(()));
        mock.expect_delete()
            .times(1)
            .returning(|_| Ok(SubstanceList::default()));
        mock.expect_select()
            .times(1)
            .returning(|_| Ok(SubstanceList::default()));
        mock.expect_query()
            .times(1)
            .returning(|_, _| Ok(QueryResult::mock()));
        mock.expect_get_properties()
            .times(1)
            .returning(|_| Ok(Properties::new()));
        mock.expect_sequence().times(1).returning(|_| Ok(64u64));
        mock.expect_set_properties()
            .times(1)
            .returning(|_, _| Ok(()));
        mock.expect_set_status().times(1).returning(|_, _| Ok(()));
        mock.expect_record()
            .times(1)
            .returning(|_| Ok(ParticleRecord::mock()));

        mock
    }

    async fn test_registry(mock: impl RegistryApi + Send + Sync + 'static) -> anyhow::Result<()> {
        mock.scorch().await.unwrap();
        mock.register(&Registration::mock()).await.unwrap();
        mock.assign_host(&Point::root(), &Point::root())
            .await
            .unwrap();
        mock.assign_star(&Point::root(), &Point::root())
            .await
            .unwrap();
        mock.delete(&Delete::mock()).await.unwrap();
        mock.select(&mut Select::mock()).await.unwrap();
        mock.query(&Point::root(), &Query::mock()).await.unwrap();
        mock.get_properties(&Point::root()).await.unwrap();
        mock.sequence(&Point::root()).await.unwrap();
        mock.set_properties(&Point::root(), &SetProperties::new())
            .await
            .unwrap();
        mock.set_status(&Point::hyper_user(), &Status::Ready)
            .await
            .unwrap();
        mock.record(&Point::global_depedencies()).await.unwrap();
        Ok(())
    }

    #[tokio::test]
    pub async fn test_mux_framed_writer() {
        let (read, write) = tokio_pipe::pipe().unwrap();

        let REQUEST: MuxedRequest = MuxedRequest::transport(1_u64,RegistryRequest::Scorch, );

        let tx = {
            let write = FramedWrite::new(
                write,
                crate::registry::exchange::MuxedRequestCodec::default(),
            );
            crate::registry::exchange::MuxFramedWriter::new(write)
        };

        let mut read = FramedRead::new(
            read,
            crate::registry::exchange::MuxedRequestCodec::default(),
        );

        let mut read = crate::registry::exchange::MuxFramedReader::new(read);

        tx.send(REQUEST.clone()).await.unwrap();

        let from_read = tokio::time::timeout(Duration::from_secs(1), read.recv())
            .await
            .unwrap()
            .unwrap();

        println!("Received FROM! {}", from_read.signal.to_string());

//        assert_eq!(REQUEST, from_read);
    }

    #[test]
    pub fn test_query_serde() {
        let query = Query::mock();
        let query = bincode::serialize(&query).unwrap();
        let query = bincode::deserialize::<Query>(&query).unwrap();
    }
}
